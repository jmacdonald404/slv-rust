//! Circuit management for Second Life UDP connections
//! 
//! Each circuit represents a connection to a specific simulator,
//! handling packet acknowledgment, retransmission, and state management.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::{Packet, PacketWrapper};
use crate::networking::serialization::{PacketSerializer, PacketDeserializer};
use crate::networking::quic_transport::QuicTransport;
use crate::networking::transport::UdpTransport;
use crate::networking::effects::{EffectManager, Position};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::timeout;
use tracing::{debug, warn, info};
use uuid::Uuid;

/// Circuit options for connecting to a simulator
#[derive(Debug, Clone)]
pub struct CircuitOptions {
    pub circuit_code: u32,
    pub address: SocketAddr,
    pub agent_id: Uuid,
    pub session_id: Uuid,
}

/// Transport type for circuit communication following ADR-0002
#[derive(Debug)]
pub enum CircuitTransport {
    /// QUIC transport with built-in reliability (preferred per ADR-0002)
    Quic(Arc<QuicTransport>),
    /// UDP transport with manual reliability (fallback)
    Udp(mpsc::UnboundedSender<(bytes::Bytes, SocketAddr)>),
}

/// Reliable packet waiting for acknowledgment
#[derive(Debug)]
struct PendingPacket {
    packet: PacketWrapper,
    sent_at: Instant,
    retry_count: u8,
    resolve_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

/// Circuit acknowledgment manager
struct Acknowledger {
    /// Packets waiting for acknowledgment (sequence -> packet)
    pending_reliable: HashMap<u32, PendingPacket>,
    /// Sequences we've received (for duplicate detection)
    received_sequences: VecDeque<u32>,
    /// Acknowledgments to send to server
    pending_acks: Vec<u32>,
    /// Maximum received sequence window size
    max_received_window: usize,
}

impl Acknowledger {
    fn new() -> Self {
        Self {
            pending_reliable: HashMap::new(),
            received_sequences: VecDeque::new(),
            pending_acks: Vec::new(),
            max_received_window: 256,
        }
    }
    
    /// Check if a sequence number is new (not already received)
    fn is_sequence_new(&mut self, sequence: u32) -> bool {
        if self.received_sequences.contains(&sequence) {
            return false;
        }
        
        // Add to received sequences
        self.received_sequences.push_back(sequence);
        
        // Maintain window size
        while self.received_sequences.len() > self.max_received_window {
            self.received_sequences.pop_front();
        }
        
        true
    }
    
    /// Queue an acknowledgment to send
    fn queue_ack(&mut self, sequence: u32) {
        self.pending_acks.push(sequence);
    }
    
    /// Get and clear pending acknowledgments
    fn take_pending_acks(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.pending_acks)
    }
    
    /// Add a reliable packet waiting for acknowledgment
    fn add_pending_reliable(&mut self, sequence: u32, packet: PacketWrapper, 
                           resolve_tx: tokio::sync::oneshot::Sender<()>) {
        let pending = PendingPacket {
            packet,
            sent_at: Instant::now(),
            retry_count: 0,
            resolve_tx: Some(resolve_tx),
        };
        self.pending_reliable.insert(sequence, pending);
    }
    
    /// Handle received acknowledgment
    fn handle_ack(&mut self, sequence: u32) -> bool {
        if let Some(mut pending) = self.pending_reliable.remove(&sequence) {
            if let Some(tx) = pending.resolve_tx.take() {
                let _ = tx.send(()); // Resolve the future
            }
            info!("✅ Reliable packet sequence {} acknowledged successfully", sequence);
            true
        } else {
            debug!("Received ACK for unknown sequence {} (already processed or expired)", sequence);
            false
        }
    }
    
    /// Get packets that need retransmission (improved exponential backoff)
    fn get_retransmits(&mut self, base_retry_timeout: Duration, max_retries: u8) -> Vec<(u32, PacketWrapper)> {
        let now = Instant::now();
        let mut retransmits = Vec::new();
        let mut to_remove = Vec::new();
        
        for (sequence, pending) in &mut self.pending_reliable {
            // WiFi-friendly exponential backoff with jitter and caps
            let exponential_factor = (1 << pending.retry_count.min(4)) as u32; // Cap at 2^4 = 16x
            let jitter = 1.0 + (pending.packet.sequence % 100) as f32 / 1000.0; // 0-10% jitter
            let retry_timeout = Duration::from_millis(
                ((base_retry_timeout.as_millis() as f32 * exponential_factor as f32 * jitter) as u64)
                    .min(15000) // Cap at 15 seconds for flaky connections
            );
            
            if now.duration_since(pending.sent_at) >= retry_timeout {
                if pending.retry_count >= max_retries {
                    // Too many retries - give up
                    warn!("Packet sequence {} failed after {} retries", sequence, pending.retry_count);
                    to_remove.push(*sequence);
                    // Don't resolve the future - let it timeout naturally
                    // This ensures the reliable send fails properly instead of false success
                } else {
                    // Retry with exponential backoff
                    pending.retry_count += 1;
                    pending.sent_at = now;
                    
                    // Create a new PacketWrapper for retransmission with resent flag
                    let mut wrapper = PacketWrapper {
                        data: pending.packet.data.clone(),
                        reliable: pending.packet.reliable,
                        resent: false, // Will be set below
                        sequence: pending.packet.sequence,
                        packet_id: pending.packet.packet_id,
                        frequency: pending.packet.frequency,
                        embedded_acks: pending.packet.embedded_acks.clone(),
                    };
                    
                    // Mark as resent (SL protocol sets RESENT flag in header)
                    // This is important for the protocol
                    wrapper.resent = true;
                    
                    retransmits.push((*sequence, wrapper));
                    
                    info!("🔄 Retransmitting packet sequence {} (attempt {}/{})", 
                          sequence, pending.retry_count, max_retries);
                }
            }
        }
        
        // Remove failed packets
        for seq in to_remove {
            self.pending_reliable.remove(&seq);
        }
        
        retransmits
    }
}

/// Circuit state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Initial state when circuit is being established
    Connecting,
    /// Circuit established but handshake not complete
    Connected,
    /// Handshake in progress
    Handshaking,
    /// Fully established and ready for communication
    Ready,
    /// Circuit experiencing degraded performance but still functional
    Degraded { 
        packet_loss_percent: f32,
        avg_rtt_ms: u32,
        reason: String,
    },
    /// Circuit is temporarily blocked due to ping failures
    Blocked,
    /// Circuit is being shut down
    Disconnecting,
    /// Circuit is fully disconnected
    Disconnected,
}

/// Ping tracking for circuit health monitoring
struct PingTracker {
    /// Outstanding pings (ping_id -> sent_time)
    outstanding_pings: HashMap<u8, Instant>,
    /// Last measured round-trip time
    last_rtt: Option<Duration>,
    /// Next ping ID to use
    next_ping_id: u8,
    /// Ping statistics
    ping_count: u32,
    blocked_pings: u32,
    /// Connection quality metrics for WiFi resilience
    rtt_history: VecDeque<Duration>,
    packet_loss_window: VecDeque<bool>, // true = success, false = loss
    avg_rtt: Duration,
    packet_loss_percent: f32,
}

impl PingTracker {
    fn new() -> Self {
        Self {
            outstanding_pings: HashMap::new(),
            last_rtt: None,
            next_ping_id: 1,
            ping_count: 0,
            blocked_pings: 0,
            rtt_history: VecDeque::new(),
            packet_loss_window: VecDeque::new(),
            avg_rtt: Duration::from_millis(100), // Default assumption
            packet_loss_percent: 0.0,
        }
    }
    
    /// Start a new ping
    fn start_ping(&mut self) -> u8 {
        let ping_id = self.next_ping_id;
        self.next_ping_id = self.next_ping_id.wrapping_add(1);
        if self.next_ping_id == 0 { self.next_ping_id = 1; } // Avoid 0
        
        self.outstanding_pings.insert(ping_id, Instant::now());
        self.ping_count += 1;
        ping_id
    }
    
    /// Complete a ping and return RTT
    fn complete_ping(&mut self, ping_id: u8) -> Option<Duration> {
        if let Some(sent_time) = self.outstanding_pings.remove(&ping_id) {
            let rtt = Instant::now().duration_since(sent_time);
            self.last_rtt = Some(rtt);
            
            // Update connection quality metrics for WiFi resilience
            self.update_quality_metrics(rtt, true);
            
            Some(rtt)
        } else {
            None
        }
    }
    
    /// Update connection quality metrics for adaptive behavior
    fn update_quality_metrics(&mut self, rtt: Duration, success: bool) {
        // Update RTT history (keep last 20 measurements)
        self.rtt_history.push_back(rtt);
        if self.rtt_history.len() > 20 {
            self.rtt_history.pop_front();
        }
        
        // Update packet loss tracking (keep last 50 attempts)
        self.packet_loss_window.push_back(success);
        if self.packet_loss_window.len() > 50 {
            self.packet_loss_window.pop_front();
        }
        
        // Calculate moving averages
        if !self.rtt_history.is_empty() {
            let total_ms: u64 = self.rtt_history.iter().map(|d| d.as_millis() as u64).sum();
            self.avg_rtt = Duration::from_millis(total_ms / self.rtt_history.len() as u64);
        }
        
        if !self.packet_loss_window.is_empty() {
            let failures = self.packet_loss_window.iter().filter(|&&s| !s).count();
            self.packet_loss_percent = (failures as f32 / self.packet_loss_window.len() as f32) * 100.0;
        }
    }
    
    /// Get connection health assessment for WiFi resilience
    fn get_connection_health(&self) -> (f32, u32, bool) {
        let is_degraded = self.packet_loss_percent > 5.0 || self.avg_rtt.as_millis() > 500;
        (self.packet_loss_percent, self.avg_rtt.as_millis() as u32, is_degraded)
    }
    
    /// Check if circuit should be considered blocked
    fn is_blocked(&self, max_outstanding: usize) -> bool {
        self.outstanding_pings.len() > max_outstanding
    }
    
    /// Clean up old pings that timed out
    fn cleanup_old_pings(&mut self, timeout: Duration) -> usize {
        let now = Instant::now();
        let mut timed_out = 0;
        
        self.outstanding_pings.retain(|_, sent_time| {
            if now.duration_since(*sent_time) > timeout {
                timed_out += 1;
                false
            } else {
                true
            }
        });
        
        // Track failed pings for connection quality metrics after retain
        for _ in 0..timed_out {
            self.update_quality_metrics(timeout, false);
        }
        
        self.blocked_pings += timed_out as u32;
        timed_out
    }
}

/// Circuit for managing connection to a Second Life simulator following ADR-0002
pub struct Circuit {
    /// Circuit configuration
    options: CircuitOptions,
    
    /// Current circuit state
    state: Arc<RwLock<CircuitState>>,
    
    /// Packet serializer
    serializer: Arc<Mutex<PacketSerializer>>,
    
    /// Packet deserializer  
    deserializer: Arc<PacketDeserializer>,
    
    /// Transport layer (QUIC or UDP per ADR-0002)
    transport: Arc<CircuitTransport>,
    
    /// Acknowledgment manager (reduced responsibility with QUIC)
    acknowledger: Arc<Mutex<Acknowledger>>,
    
    /// Ping tracker for circuit health
    ping_tracker: Arc<Mutex<PingTracker>>,
    
    /// Effect manager for ViewerEffect messages
    effect_manager: Arc<Mutex<EffectManager>>,
    
    /// Channel for receiving packets from transport
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<PacketWrapper>>>,
    
    /// Channel for outgoing events
    event_tx: mpsc::UnboundedSender<CircuitEvent>,
    
    /// Retry configuration (used only for UDP fallback)
    retry_timeout: Duration,
    max_retries: u8,
    
    /// Ping configuration
    ping_interval: Duration,
    ping_timeout: Duration,
    max_outstanding_pings: usize,
}

impl std::fmt::Debug for Circuit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Circuit")
            .field("options", &self.options)
            .field("retry_timeout", &self.retry_timeout)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

use bytes::Bytes;

/// Events emitted by the circuit
#[derive(Debug, Clone)]
pub enum CircuitEvent {
    /// Circuit connected successfully
    Connected,
    /// Circuit disconnected
    Disconnected { reason: String },
    /// Packet received
    PacketReceived { packet: PacketWrapper },
    /// Error occurred
    Error { error: NetworkError },
}

impl Circuit {
    /// Create a new circuit with QUIC transport (preferred per ADR-0002)
    pub fn new_with_quic(
        options: CircuitOptions,
        transport: Arc<QuicTransport>,
        packet_rx: mpsc::UnboundedReceiver<PacketWrapper>,
        event_tx: mpsc::UnboundedSender<CircuitEvent>,
    ) -> Self {
        info!("🔐 Creating circuit with QUIC transport for {} (ADR-0002)", options.address);
        Self {
            options,
            state: Arc::new(RwLock::new(CircuitState::Connecting)),
            serializer: Arc::new(Mutex::new(PacketSerializer::new())),
            deserializer: Arc::new(PacketDeserializer::new()),
            transport: Arc::new(CircuitTransport::Quic(transport)),
            acknowledger: Arc::new(Mutex::new(Acknowledger::new())),
            ping_tracker: Arc::new(Mutex::new(PingTracker::new())),
            effect_manager: Arc::new(Mutex::new(EffectManager::new())),
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            event_tx,
            retry_timeout: Duration::from_millis(1500), // Faster initial retry for flaky WiFi
            max_retries: 8, // More retries for unstable connections
            ping_interval: Duration::from_secs(45), // More frequent keepalives
            ping_timeout: Duration::from_secs(20), // Shorter ping timeout
            max_outstanding_pings: 3, // Fewer concurrent pings
        }
    }
    
    /// Create a new circuit with UDP transport (fallback)
    pub fn new_with_udp(
        options: CircuitOptions,
        packet_tx: mpsc::UnboundedSender<(bytes::Bytes, SocketAddr)>,
        packet_rx: mpsc::UnboundedReceiver<PacketWrapper>,
        event_tx: mpsc::UnboundedSender<CircuitEvent>,
    ) -> Self {
        info!("🔄 Creating circuit with UDP transport (fallback) for {}", options.address);
        Self {
            options,
            state: Arc::new(RwLock::new(CircuitState::Connecting)),
            serializer: Arc::new(Mutex::new(PacketSerializer::new())),
            deserializer: Arc::new(PacketDeserializer::new()),
            transport: Arc::new(CircuitTransport::Udp(packet_tx)),
            acknowledger: Arc::new(Mutex::new(Acknowledger::new())),
            ping_tracker: Arc::new(Mutex::new(PingTracker::new())),
            effect_manager: Arc::new(Mutex::new(EffectManager::new())),
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            event_tx,
            retry_timeout: Duration::from_millis(1500), // Faster initial retry for flaky WiFi
            max_retries: 8, // More retries for unstable connections
            ping_interval: Duration::from_secs(45), // More frequent keepalives
            ping_timeout: Duration::from_secs(20), // Shorter ping timeout
            max_outstanding_pings: 3, // Fewer concurrent pings
        }
    }
    
    /// Get circuit address
    pub fn address(&self) -> SocketAddr {
        self.options.address
    }
    
    /// Get adaptive timeout based on connection health for WiFi resilience
    async fn get_adaptive_timeout(&self, base_timeout: Duration) -> Duration {
        let tracker = self.ping_tracker.lock().await;
        let (packet_loss, avg_rtt, is_degraded) = tracker.get_connection_health();
        
        if is_degraded {
            // Increase timeout for degraded connections
            let multiplier = if packet_loss > 10.0 {
                3.0 // High packet loss - much longer timeout
            } else if avg_rtt > 1000 {
                2.5 // High latency - longer timeout  
            } else {
                1.5 // Mildly degraded - slightly longer
            };
            
            Duration::from_millis((base_timeout.as_millis() as f32 * multiplier) as u64)
                .min(Duration::from_secs(60)) // Cap at 60 seconds
        } else {
            // Healthy connection - use base timeout
            base_timeout
        }
    }
    
    /// Get circuit code
    pub fn circuit_code(&self) -> u32 {
        self.options.circuit_code
    }
    
    /// Get current state
    pub async fn state(&self) -> CircuitState {
        self.state.read().await.clone()
    }
    
    /// Set circuit state and emit events
    pub async fn set_state(&self, new_state: CircuitState) -> NetworkResult<()> {
        let old_state = {
            let mut state = self.state.write().await;
            let old = state.clone();
            *state = new_state.clone();
            old
        };
        
        if old_state != new_state {
            info!("Circuit {} state changed: {:?} -> {:?}", self.options.address, old_state, new_state);
            
            // Emit state change events
            match new_state {
                CircuitState::Ready => {
                    let _ = self.event_tx.send(CircuitEvent::Connected);
                },
                CircuitState::Disconnected => {
                    let _ = self.event_tx.send(CircuitEvent::Disconnected { 
                        reason: "State transition".to_string() 
                    });
                },
                _ => {} // No event for intermediate states
            }
        }
        
        Ok(())
    }
    
    /// Send a packet (unreliable) - optimized for QUIC datagrams per ADR-0002
    pub async fn send<P: Packet>(&self, packet: &P) -> NetworkResult<()> {
        let mut serializer = self.serializer.lock().await;
        let (data, _) = serializer.serialize(packet, false)?;
        
        match &*self.transport.as_ref() {
            CircuitTransport::Quic(quic_transport) => {
                // Use QUIC unreliable datagrams for non-critical packets (optimized)
                quic_transport.send_unreliable(data, self.options.address).await
            },
            CircuitTransport::Udp(packet_tx) => {
                // Fallback to UDP
                packet_tx.send((data, self.options.address))
                    .map_err(|_| NetworkError::ConnectionLost { 
                        address: self.options.address 
                    })?;
                Ok(())
            }
        }
    }
    
    /// Send a packet reliably - leverages QUIC built-in reliability per ADR-0002
    /// SECURITY: Enhanced reliability with QUIC streams for critical authentication packets
    pub async fn send_reliable<P: Packet>(&self, packet: &P, timeout_duration: Duration) -> NetworkResult<()> {
        // Adaptive timeout based on connection health for WiFi resilience
        let adaptive_timeout = self.get_adaptive_timeout(timeout_duration).await;
        // SECURITY: Validate circuit state before sending sensitive packets
        let current_state = self.state.read().await.clone();
        if matches!(current_state, CircuitState::Disconnected | CircuitState::Disconnecting) {
            return Err(NetworkError::ConnectionLost { 
                address: self.options.address 
            });
        }
        
        let mut serializer = self.serializer.lock().await;
        let (data, sequence) = serializer.serialize(packet, true)?;
        
        info!("🔒 CIRCUIT SEND: Sending SECURE reliable packet");
        info!("   Size: {} bytes", data.len());
        info!("   Sequence: {}", sequence);
        info!("   Destination: {}", self.options.address);
        info!("   Circuit State: {:?}", current_state);
        info!("   Timeout: {:?}", adaptive_timeout);
        
        let send_start = std::time::Instant::now();
        
        match &*self.transport.as_ref() {
            CircuitTransport::Quic(quic_transport) => {
                // QUIC provides built-in reliability, so we can send directly
                // No need for manual ACK tracking - QUIC handles this at the transport layer
                info!("🔐 CIRCUIT SEND: Using QUIC built-in reliability for secure packet transmission");
                
                match timeout(adaptive_timeout, quic_transport.send_reliable(data, self.options.address)).await {
                    Ok(Ok(())) => {
                        let send_time = send_start.elapsed();
                        info!("✅ CIRCUIT RESPONSE: SECURE reliable packet sent via QUIC stream");
                        info!("   Sequence: {}", sequence);
                        info!("   Send time: {:?}", send_time);
                        info!("   Adaptive timeout: {:?}", adaptive_timeout);
                        Ok(())
                    },
                    Ok(Err(e)) => {
                        let send_time = send_start.elapsed();
                        warn!("❌ CIRCUIT RESPONSE ERROR: QUIC reliable send failed");
                        warn!("   Sequence: {}", sequence);
                        warn!("   Error: {}", e);
                        warn!("   Send time: {:?}", send_time);
                        Err(e)
                    },
                    Err(_) => {
                        let send_time = send_start.elapsed();
                        warn!("⏰ CIRCUIT RESPONSE TIMEOUT: QUIC reliable send timeout");
                        warn!("   Sequence: {}", sequence);
                        warn!("   Timeout after: {:?}", send_time);
                        warn!("   Adaptive timeout was: {:?}", adaptive_timeout);
                        Err(NetworkError::HandshakeTimeout)
                    }
                }
            },
            CircuitTransport::Udp(packet_tx) => {
                // Fallback to UDP with manual acknowledgment tracking
                info!("🔄 Using UDP fallback with manual ACK tracking for reliable packet");
                
                let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel();
                
                // Create packet wrapper for acknowledgment tracking
                let wrapper = PacketWrapper::new(packet, Some(true))?;
                
                // Add to pending reliable packets
                {
                    let mut ack = self.acknowledger.lock().await;
                    ack.add_pending_reliable(sequence, wrapper, resolve_tx);
                }
                
                // Send the packet
                packet_tx.send((data, self.options.address))
                    .map_err(|_| NetworkError::ConnectionLost { 
                        address: self.options.address 
                    })?;
                
                // Wait for acknowledgment with adaptive timeout  
                match timeout(adaptive_timeout, resolve_rx).await {
                    Ok(Ok(())) => {
                        info!("✅ SECURE reliable packet acknowledged via UDP: sequence {} ({}s adaptive timeout)", 
                              sequence, adaptive_timeout.as_secs());
                        Ok(())
                    },
                    Ok(Err(_)) => {
                        warn!("🔒 SECURITY: UDP reliable packet channel closed unexpectedly for sequence {}", sequence);
                        Err(NetworkError::HandshakeTimeout)
                    },
                    Err(_) => {
                        warn!("🔒 SECURITY: UDP reliable packet acknowledgment timeout for sequence {} after {}s (adaptive)", 
                              sequence, adaptive_timeout.as_secs());
                        Err(NetworkError::HandshakeTimeout)
                    },
                }
            }
        }
    }
    
    /// Process incoming packet and handle acknowledgments
    pub async fn handle_incoming_packet(&self, packet_data: &[u8]) -> NetworkResult<()> {
        let mut deserializer = PacketDeserializer::new();
        
        // Parse packet header to extract sequence number and flags
        if packet_data.len() < 6 {
            return Err(NetworkError::Transport { 
                reason: "Packet too short for header".to_string() 
            });
        }
        
        let flags = packet_data[0];
        let sequence = u32::from_le_bytes([
            packet_data[1], packet_data[2], packet_data[3], packet_data[4]
        ]);
        
        // Check if this is a reliable packet that needs acknowledgment
        if (flags & 0x40) != 0 { // ACK_FLAG
            let mut ack = self.acknowledger.lock().await;
            if ack.is_sequence_new(sequence) {
                ack.queue_ack(sequence);
                info!("📥 Queued ACK for reliable packet sequence {}", sequence);
            }
        }
        
        // Check if this is a PacketAck message
        let packet_id = if packet_data.len() >= 7 {
            if packet_data[5] == 0xFF && packet_data.len() >= 10 {
                // Extended packet ID format
                u32::from_le_bytes([packet_data[6], packet_data[7], packet_data[8], packet_data[9]]) & 0xFFFF
            } else {
                packet_data[6] as u32
            }
        } else {
            return Ok(()); // Skip malformed packets
        };
        
        // PacketAck has ID 0xFFFFFFFB (4294967291)
        if packet_id == 0xFFFFFFFB {
            self.handle_packet_ack(packet_data).await?;
        }
        
        Ok(())
    }
    
    /// Handle PacketAck message from server  
    /// NOTE: This method is deprecated - PacketAck messages are now handled by PacketAckHandler
    async fn handle_packet_ack(&self, packet_data: &[u8]) -> NetworkResult<()> {
        warn!("handle_packet_ack called - this should not happen as PacketAck is handled by PacketAckHandler");
        Ok(())
    }
    
    /// Simplified acknowledgment for UseCircuitCode - acknowledges first pending packet
    /// This is a workaround until full packet routing is implemented
    pub async fn acknowledge_first_pending(&self) -> bool {
        let mut ack = self.acknowledger.lock().await;
        if let Some(&sequence) = ack.pending_reliable.keys().next() {
            ack.handle_ack(sequence);
            info!("✅ Acknowledged first pending packet (sequence {})", sequence);
            true
        } else {
            false
        }
    }
    
    /// Start the circuit (begin packet processing)
    pub async fn start(&self) -> NetworkResult<()> {
        self.set_state(CircuitState::Connected).await?;
        
        // Start background tasks
        self.start_retry_handler().await;
        self.start_ack_sender().await;
        self.start_ping_handler().await;
        
        info!("Circuit {} started successfully", self.options.address);        
        Ok(())
    }
    
    /// Stop the circuit
    pub async fn stop(&self) -> NetworkResult<()> {
        {
            let mut state = self.state.write().await;
            *state = CircuitState::Disconnecting;
        }
        
        // Send logout request if connected
        if *self.state.read().await == CircuitState::Connected {
            use crate::networking::packets::generated::*;
            let logout = LogoutRequest {
                agent_data: LogoutRequestAgentDataBlock {
                    agent_id: self.options.agent_id,
                    session_id: self.options.session_id,
                },
            };
            let _ = self.send_reliable(&logout, Duration::from_secs(5)).await;
        }
        
        {
            let mut state = self.state.write().await;
            *state = CircuitState::Disconnected;
        }
        
        let _ = self.event_tx.send(CircuitEvent::Disconnected { 
            reason: "User requested".to_string() 
        });
        
        Ok(())
    }
    
    
    /// Start retry handler task (UDP fallback only - QUIC handles retries internally)
    async fn start_retry_handler(&self) {
        let acknowledger = Arc::clone(&self.acknowledger);
        let transport = Arc::clone(&self.transport);
        let serializer = Arc::clone(&self.serializer);
        let address = self.options.address;
        let retry_timeout = self.retry_timeout;
        let max_retries = self.max_retries;
        let state = Arc::clone(&self.state);
        
        // Only start retry handler for UDP transport (QUIC handles retries internally)
        if matches!(&*transport.as_ref(), CircuitTransport::Udp(_)) {
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(retry_timeout);
                
                loop {
                    interval.tick().await;
                    
                    // Check if circuit is still active
                    if *state.read().await == CircuitState::Disconnected {
                        break;
                    }
                    
                    // Check for packets needing retransmission
                    let retransmits = {
                        let mut ack = acknowledger.lock().await;
                        ack.get_retransmits(retry_timeout, max_retries)
                    };
                    
                    // Retransmit packets via UDP transport
                    if let CircuitTransport::Udp(packet_tx) = &*transport.as_ref() {
                        for (sequence, wrapper) in retransmits {
                            let mut serializer = serializer.lock().await;
                            if let Ok(data) = serializer.serialize_wrapper(&wrapper) {
                                let data_len = data.len();
                                let _ = packet_tx.send((data, address));
                                info!("📤 Retransmitted packet sequence {} to {} ({} bytes, marked as RESENT)", 
                                      sequence, address, data_len);
                            } else {
                                warn!("❌ Failed to serialize packet for retransmission: sequence {}", sequence);
                            }
                        }
                    }
                }
            });
        } else {
            debug!("🔐 QUIC transport - retry handler not needed (built-in reliability)");
        }
    }
    
    /// Start acknowledgment sender task (UDP fallback only - QUIC handles ACKs internally)
    async fn start_ack_sender(&self) {
        let acknowledger = Arc::clone(&self.acknowledger);
        let transport = Arc::clone(&self.transport);
        let serializer = Arc::clone(&self.serializer);
        let address = self.options.address;
        let state = Arc::clone(&self.state);
        
        // PROTOCOL COMPLIANCE: PacketAck is required at application layer for both transports
        // Even with QUIC, SL protocol expects PacketAck messages for reliable packets
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(200));
            
            loop {
                interval.tick().await;
                
                // Check if circuit is still active
                if matches!(*state.read().await, CircuitState::Disconnected) {
                    break;
                }
                
                // Get pending acknowledgments
                let acks = {
                    let mut ack = acknowledger.lock().await;
                    ack.take_pending_acks()
                };
                
                // Send acknowledgment packet if we have any - PROTOCOL COMPLIANCE REQUIRES THIS
                if !acks.is_empty() {
                    use crate::networking::packets::generated::*;
                    
                    let packet_ack = PacketAck {
                        packets: acks.into_iter()
                            .map(|id| PacketAckPacketsBlock { id })
                            .collect(),
                    };
                    
                    match &*transport.as_ref() {
                        CircuitTransport::Udp(packet_tx) => {
                            let mut serializer = serializer.lock().await;
                            if let Ok((data, _)) = serializer.serialize(&packet_ack, false) {
                                let _ = packet_tx.send((data, address));
                                info!("📮 Sent {} PacketAck(s) via UDP for protocol compliance", packet_ack.packets.len());
                            }
                        },
                        CircuitTransport::Quic(quic_transport) => {
                            // PROTOCOL COMPLIANCE: Send PacketAck even over QUIC (application layer requirement)
                            let mut serializer = serializer.lock().await;
                            if let Ok((data, _)) = serializer.serialize(&packet_ack, false) {
                                if let Err(e) = quic_transport.get_sender().send((data, address)) {
                                    warn!("Failed to send PacketAck via QUIC: {}", e);
                                } else {
                                    info!("📮 Sent {} PacketAck(s) via QUIC for protocol compliance", packet_ack.packets.len());
                                }
                            }
                        }
                    }
                }
            }
        });
    }
    
    /// Handle received acknowledgment packet
    pub async fn handle_ack(&self, ack_packet: &crate::networking::packets::generated::PacketAck) {
        let mut ack = self.acknowledger.lock().await;
        info!("Processing {} acknowledgments", ack_packet.packets.len());
        for packet_block in &ack_packet.packets {
            let was_pending = ack.handle_ack(packet_block.id);
            if was_pending {
                info!("Acknowledged reliable packet with sequence {}", packet_block.id);
            } else {
                debug!("Received ACK for unknown/expired sequence {}", packet_block.id);
            }
        }
    }
    
    /// Send a ping to check circuit health
    pub async fn send_ping(&self) -> NetworkResult<u8> {
        use crate::networking::packets::generated::*;
        
        let ping_id = {
            let mut tracker = self.ping_tracker.lock().await;
            tracker.start_ping()
        };
        
        let ping_packet = StartPingCheck { 
            ping_id: StartPingCheckPingIDBlock {
                ping_id,
                oldest_unacked: 0, // Empty for now - could track oldest unacked sequence
            },
        };
        
        // Send ping unreliably - pings are frequent and don't need to be reliable
        self.send(&ping_packet).await?;
        info!("Sent ping {} to {}", ping_id, self.options.address);
        
        Ok(ping_id)
    }
    
    /// Handle a ping response
    pub async fn handle_ping_response(&self, ping_id: u8) -> Option<Duration> {
        let mut tracker = self.ping_tracker.lock().await;
        let rtt = tracker.complete_ping(ping_id);
        
        if let Some(rtt) = rtt {
            info!("Ping {} completed in {:?}", ping_id, rtt);
        } else {
            warn!("Received ping response for unknown ping ID: {}", ping_id);
        }
        
        rtt
    }
    
    /// Check if circuit is blocked due to ping failures
    pub async fn is_blocked(&self) -> bool {
        let tracker = self.ping_tracker.lock().await;
        tracker.is_blocked(self.max_outstanding_pings)
    }
    
    /// Get circuit health statistics
    pub async fn get_ping_stats(&self) -> (Option<Duration>, u32, u32, usize) {
        let tracker = self.ping_tracker.lock().await;
        (tracker.last_rtt, tracker.ping_count, tracker.blocked_pings, tracker.outstanding_pings.len())
    }
    
    /// Start ping handler task
    async fn start_ping_handler(&self) {
        let ping_tracker = Arc::clone(&self.ping_tracker);
        let transport = Arc::clone(&self.transport);
        let serializer = Arc::clone(&self.serializer);
        let address = self.options.address;
        let state = Arc::clone(&self.state);
        let ping_interval = self.ping_interval;
        let ping_timeout = self.ping_timeout;
        let max_outstanding = self.max_outstanding_pings;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(ping_interval);
            
            loop {
                interval.tick().await;
                
                // Check if circuit is still active
                if *state.read().await == CircuitState::Disconnected {
                    break;
                }
                
                // Clean up old pings and check for blocking + WiFi health
                let (timed_out, is_blocked, connection_health) = {
                    let mut tracker = ping_tracker.lock().await;
                    let timed_out = tracker.cleanup_old_pings(ping_timeout);
                    let is_blocked = tracker.is_blocked(max_outstanding);
                    let health = tracker.get_connection_health();
                    (timed_out, is_blocked, health)
                };
                
                let (packet_loss, avg_rtt, is_degraded) = connection_health;
                
                if timed_out > 0 {
                    warn!("Circuit {} had {} ping timeouts", address, timed_out);
                }
                
                // WiFi-aware state management
                match *state.read().await {
                    CircuitState::Ready | CircuitState::Connected | CircuitState::Handshaking => {
                        if is_blocked {
                            warn!("Circuit {} blocked: {} ping failures, switching to BLOCKED", address, max_outstanding);
                            let mut state_lock = state.write().await;
                            *state_lock = CircuitState::Blocked;
                        } else if is_degraded {
                            info!("Circuit {} degraded: {:.1}% loss, {}ms RTT", address, packet_loss, avg_rtt);
                            let mut state_lock = state.write().await;
                            *state_lock = CircuitState::Degraded {
                                packet_loss_percent: packet_loss,
                                avg_rtt_ms: avg_rtt,
                                reason: if packet_loss > 5.0 { 
                                    "High packet loss".to_string() 
                                } else { 
                                    "High latency".to_string() 
                                },
                            };
                        }
                    },
                    CircuitState::Degraded { .. } => {
                        if is_blocked {
                            warn!("Circuit {} degraded→blocked: too many failures", address);
                            let mut state_lock = state.write().await;
                            *state_lock = CircuitState::Blocked;
                        } else if !is_degraded {
                            info!("Circuit {} recovered: {:.1}% loss, {}ms RTT", address, packet_loss, avg_rtt);
                            let mut state_lock = state.write().await;
                            *state_lock = CircuitState::Ready;
                        }
                    },
                    CircuitState::Blocked => {
                        if !is_blocked && !is_degraded {
                            info!("Circuit {} unblocked: connection recovered", address);
                            let mut state_lock = state.write().await;
                            *state_lock = CircuitState::Ready;
                        }
                    },
                    _ => {
                        // Other states (Connecting, Disconnecting, etc.) - no ping monitoring
                    }
                }
                
                // Send a new ping
                let ping_id = {
                    let mut tracker = ping_tracker.lock().await;
                    tracker.start_ping()
                };
                
                use crate::networking::packets::generated::*;
                let ping_packet = StartPingCheck { 
            ping_id: StartPingCheckPingIDBlock {
                ping_id,
                oldest_unacked: 0, // Empty for now - could track oldest unacked sequence
            },
        };
                
                let mut serializer = serializer.lock().await;
                if let Ok((data, _)) = serializer.serialize(&ping_packet, false) {
                    match &*transport.as_ref() {
                        CircuitTransport::Quic(quic_transport) => {
                            if let Err(e) = quic_transport.send_unreliable(data.into(), address).await {
                                warn!("Failed to send ping {} via QUIC: {}", ping_id, e);
                            } else {
                                debug!("Sent ping {} to {} via QUIC", ping_id, address);
                            }
                        }
                        CircuitTransport::Udp(packet_tx) => {
                            let _ = packet_tx.send((data, address));
                            debug!("Sent ping {} to {} via UDP", ping_id, address);
                        }
                    }
                } else {
                    warn!("Failed to serialize ping packet");
                }
            }
        });
    }
    
    /// Inject a packet received from the transport layer (like homunculus circuit.receive)
    pub async fn inject_packet(&self, packet: PacketWrapper) -> NetworkResult<()> {
        // Following homunculus circuit.receive pattern (lines 107-120)
        
        info!("Circuit {} received packet: id={}, frequency={:?}, reliable={}", 
              self.options.address, packet.packet_id, packet.frequency, packet.reliable);
        
        // Handle reliable packets - queue acknowledgment if new
        if packet.reliable {
            let mut ack = self.acknowledger.lock().await;
            if !ack.is_sequence_new(packet.sequence) {
                // Ignore packets we've already seen (like homunculus line 110)
                info!("Ignoring duplicate reliable packet with sequence {}", packet.sequence);
                return Ok(());
            }
            ack.queue_ack(packet.sequence);
        }
        
        // Handle acknowledgments embedded in this packet (like homunculus lines 116-120)
        // Note: In SL protocol, packets can carry acknowledgments for other packets
        // This is critical for resolving pending reliable packet promises
        
        // Check if this packet contains acknowledgments in the header
        // SL protocol allows packets to carry acknowledgment sequences in the header
        if let Some(ack_list) = packet.embedded_acks.as_ref() {
            let mut ack = self.acknowledger.lock().await;
            for &ack_seq in ack_list {
                let was_pending = ack.handle_ack(ack_seq);
                if was_pending {
                    info!("Processed embedded ACK for sequence {}", ack_seq);
                }
            }
        }
        
        // Check if this is a dedicated PacketAck message 
        // Construct full packet ID like the handler system does
        let full_packet_id = match packet.frequency {
            crate::networking::packets::PacketFrequency::High => packet.packet_id as u32,
            crate::networking::packets::PacketFrequency::Medium => (1 << 16) | (packet.packet_id as u32),
            crate::networking::packets::PacketFrequency::Low => (2 << 16) | (packet.packet_id as u32),
            crate::networking::packets::PacketFrequency::Fixed => (3 << 16) | (packet.packet_id as u32),
        };
        
        // PacketAck messages will be handled by the PacketAckHandler via the event system
        // No need for double processing here - let the proper handler do the work
        
        // Emit packet received event - this will trigger the handlers
        let _ = self.event_tx.send(CircuitEvent::PacketReceived { packet });
        Ok(())
    }

    /// Send a ViewerEffect message (point-at gesture)
    pub async fn send_point_at_effect(
        &self, 
        source_pos: Position, 
        target_pos: Position
    ) -> NetworkResult<()> {
        use crate::networking::packets::generated::ViewerEffect;
        
        let mut effect_manager = self.effect_manager.lock().await;
        let effect_message = effect_manager.create_point_at_effect(
            self.options.agent_id,
            self.options.session_id,
            source_pos,
            target_pos,
        );

        info!("👉 Sending point-at effect from {:?} to {:?}", 
              (source_pos.x, source_pos.y, source_pos.z),
              (target_pos.x, target_pos.y, target_pos.z));

        // Send as unreliable packet (effects don't need guaranteed delivery)
        self.send(&effect_message).await
    }

    /// Send a ViewerEffect message (beam effect)
    pub async fn send_beam_effect(
        &self,
        source_pos: Position,
        target_pos: Position,
    ) -> NetworkResult<()> {
        let mut effect_manager = self.effect_manager.lock().await;
        let effect_message = effect_manager.create_beam_effect(
            self.options.agent_id,
            self.options.session_id,
            source_pos,
            target_pos,
        );

        info!("⚡ Sending beam effect from {:?} to {:?}", 
              (source_pos.x, source_pos.y, source_pos.z),
              (target_pos.x, target_pos.y, target_pos.z));

        // Send as unreliable packet
        self.send(&effect_message).await
    }

    /// Send a generic ViewerEffect message
    pub async fn send_viewer_effect(&self, config: crate::networking::effects::EffectConfig) -> NetworkResult<()> {
        let mut effect_manager = self.effect_manager.lock().await;
        let effect_message = effect_manager.create_viewer_effect(self.options.session_id, config);

        info!("🎭 Sending viewer effect: {:?}", effect_message.effect[0].r#type);
        
        // Send as unreliable packet
        self.send(&effect_message).await
    }

    /// Cleanup expired effects (should be called periodically)
    pub async fn cleanup_effects(&self) {
        let mut effect_manager = self.effect_manager.lock().await;
        effect_manager.cleanup_expired_effects();
    }
}