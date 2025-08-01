//! Circuit management for Second Life UDP connections
//! 
//! Each circuit represents a connection to a specific simulator,
//! handling packet acknowledgment, retransmission, and state management.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::{Packet, PacketWrapper};
use crate::networking::serialization::{PacketSerializer, PacketDeserializer};
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
            true
        } else {
            false
        }
    }
    
    /// Get packets that need retransmission (improved exponential backoff)
    fn get_retransmits(&mut self, base_retry_timeout: Duration, max_retries: u8) -> Vec<(u32, PacketWrapper)> {
        let now = Instant::now();
        let mut retransmits = Vec::new();
        let mut to_remove = Vec::new();
        
        for (sequence, pending) in &mut self.pending_reliable {
            // Exponential backoff: 2^retry_count * base_timeout
            let retry_timeout = base_retry_timeout * (1 << pending.retry_count.min(6)) as u32;
            
            if now.duration_since(pending.sent_at) >= retry_timeout {
                if pending.retry_count >= max_retries {
                    // Too many retries - give up
                    warn!("Packet sequence {} failed after {} retries", sequence, pending.retry_count);
                    to_remove.push(*sequence);
                    if let Some(tx) = pending.resolve_tx.take() {
                        let _ = tx.send(()); // Resolve with failure (could be error instead)
                    }
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
}

impl PingTracker {
    fn new() -> Self {
        Self {
            outstanding_pings: HashMap::new(),
            last_rtt: None,
            next_ping_id: 1,
            ping_count: 0,
            blocked_pings: 0,
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
            Some(rtt)
        } else {
            None
        }
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
        
        self.blocked_pings += timed_out as u32;
        timed_out
    }
}

/// Circuit for managing connection to a Second Life simulator
pub struct Circuit {
    /// Circuit configuration
    options: CircuitOptions,
    
    /// Current circuit state
    state: Arc<RwLock<CircuitState>>,
    
    /// Packet serializer
    serializer: Arc<Mutex<PacketSerializer>>,
    
    /// Packet deserializer  
    deserializer: Arc<PacketDeserializer>,
    
    /// Acknowledgment manager
    acknowledger: Arc<Mutex<Acknowledger>>,
    
    /// Ping tracker for circuit health
    ping_tracker: Arc<Mutex<PingTracker>>,
    
    /// Channel for sending packets to transport
    packet_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
    
    /// Channel for receiving packets from transport
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<PacketWrapper>>>,
    
    /// Channel for outgoing events
    event_tx: mpsc::UnboundedSender<CircuitEvent>,
    
    /// Retry configuration
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
    pub fn new(
        options: CircuitOptions,
        packet_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
        packet_rx: mpsc::UnboundedReceiver<PacketWrapper>,
        event_tx: mpsc::UnboundedSender<CircuitEvent>,
    ) -> Self {
        Self {
            options,
            state: Arc::new(RwLock::new(CircuitState::Connecting)),
            serializer: Arc::new(Mutex::new(PacketSerializer::new())),
            deserializer: Arc::new(PacketDeserializer::new()),
            acknowledger: Arc::new(Mutex::new(Acknowledger::new())),
            ping_tracker: Arc::new(Mutex::new(PingTracker::new())),
            packet_tx,
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            event_tx,
            retry_timeout: Duration::from_secs(3),
            max_retries: 3,
            ping_interval: Duration::from_secs(60), // Ping every 60 seconds
            ping_timeout: Duration::from_secs(30),  // 30 second ping timeout
            max_outstanding_pings: 5,              // Max 5 outstanding pings before blocking
        }
    }
    
    /// Get circuit address
    pub fn address(&self) -> SocketAddr {
        self.options.address
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
    
    /// Send a packet (unreliable)
    pub async fn send<P: Packet>(&self, packet: &P) -> NetworkResult<()> {
        let mut serializer = self.serializer.lock().await;
        let (data, _) = serializer.serialize(packet, false)?;
        
        self.packet_tx.send((data, self.options.address))
            .map_err(|_| NetworkError::ConnectionLost { 
                address: self.options.address 
            })?;
            
        Ok(())
    }
    
    /// Send a packet reliably (with acknowledgment)
    /// SECURITY: Enhanced reliability with retry mechanism for critical authentication packets
    pub async fn send_reliable<P: Packet>(&self, packet: &P, timeout_duration: Duration) -> NetworkResult<()> {
        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel();
        
        // SECURITY: Validate circuit state before sending sensitive packets
        let current_state = self.state.read().await.clone();
        if matches!(current_state, CircuitState::Disconnected | CircuitState::Disconnecting) {
            return Err(NetworkError::ConnectionLost { 
                address: self.options.address 
            });
        }
        
        let mut serializer = self.serializer.lock().await;
        let (data, sequence) = serializer.serialize(packet, true)?;
        
        info!("🔒 Sending SECURE reliable packet: {} bytes, sequence {}, to {} (state: {:?})", 
              data.len(), sequence, self.options.address, current_state);
        
        // Create packet wrapper for acknowledgment tracking
        let wrapper = PacketWrapper::new(packet, Some(true))?;
        
        // Add to pending reliable packets
        {
            let mut ack = self.acknowledger.lock().await;
            ack.add_pending_reliable(sequence, wrapper, resolve_tx);
        }
        
        // Send the packet
        self.packet_tx.send((data, self.options.address))
            .map_err(|_| NetworkError::ConnectionLost { 
                address: self.options.address 
            })?;
        
        // Wait for acknowledgment with timeout - enhanced error reporting for security
        match timeout(timeout_duration, resolve_rx).await {
            Ok(Ok(())) => {
                info!("✅ SECURE reliable packet acknowledged: sequence {} ({}s timeout)", 
                      sequence, timeout_duration.as_secs());
                Ok(())
            },
            Ok(Err(_)) => {
                warn!("🔒 SECURITY: Reliable packet channel closed unexpectedly for sequence {} - possible connection compromise", sequence);
                Err(NetworkError::HandshakeTimeout)
            },
            Err(_) => {
                warn!("🔒 SECURITY: Reliable packet acknowledgment timeout for sequence {} after {}s - connection may be compromised", 
                      sequence, timeout_duration.as_secs());
                Err(NetworkError::HandshakeTimeout)
            },
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
                agent_id: self.options.agent_id,
                session_id: self.options.session_id,
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
    
    
    /// Start retry handler task
    async fn start_retry_handler(&self) {
        let acknowledger = Arc::clone(&self.acknowledger);
        let packet_tx = self.packet_tx.clone();
        let serializer = Arc::clone(&self.serializer);
        let address = self.options.address;
        let retry_timeout = self.retry_timeout;
        let max_retries = self.max_retries;
        let state = Arc::clone(&self.state);
        
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
                
                // Retransmit packets
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
        });
    }
    
    /// Start acknowledgment sender task
    async fn start_ack_sender(&self) {
        let acknowledger = Arc::clone(&self.acknowledger);
        let packet_tx = self.packet_tx.clone();
        let serializer = Arc::clone(&self.serializer);
        let address = self.options.address;
        let state = Arc::clone(&self.state);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            
            loop {
                interval.tick().await;
                
                // Check if circuit is still active
                if *state.read().await == CircuitState::Disconnected {
                    break;
                }
                
                // Get pending acknowledgments
                let acks = {
                    let mut ack = acknowledger.lock().await;
                    ack.take_pending_acks()
                };
                
                // Send acknowledgment packet if we have any
                if !acks.is_empty() {
                    use crate::networking::packets::generated::*;
                    
                    let packet_ack = PacketAck {
                        packets: acks.into_iter()
                            .map(|id| PacketsBlock { id })
                            .collect(),
                    };
                    
                    let mut serializer = serializer.lock().await;
                    if let Ok((data, _)) = serializer.serialize(&packet_ack, false) {
                        let _ = packet_tx.send((data, address));
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
            ping_id,
            oldest_unacked: Vec::new(), // Empty for now - could track oldest unacked sequence
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
        let packet_tx = self.packet_tx.clone();
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
                
                // Clean up old pings and check for blocking
                let (timed_out, is_blocked) = {
                    let mut tracker = ping_tracker.lock().await;
                    let timed_out = tracker.cleanup_old_pings(ping_timeout);
                    let is_blocked = tracker.is_blocked(max_outstanding);
                    (timed_out, is_blocked)
                };
                
                if timed_out > 0 {
                    warn!("Circuit {} had {} ping timeouts", address, timed_out);
                }
                
                if is_blocked {
                    warn!("Circuit {} is blocked due to ping failures", address);
                    // Set circuit to blocked state (SL protocol pauses agents when blocked)
                    if let Ok(current_state) = state.try_read() {
                        if *current_state != CircuitState::Blocked {
                            drop(current_state);
                            let mut state_lock = state.write().await;
                            *state_lock = CircuitState::Blocked;
                            warn!("Circuit {} set to BLOCKED state", address);
                        }
                    }
                    continue;
                }
                
                // Send a new ping
                let ping_id = {
                    let mut tracker = ping_tracker.lock().await;
                    tracker.start_ping()
                };
                
                use crate::networking::packets::generated::*;
                let ping_packet = StartPingCheck { 
            ping_id,
            oldest_unacked: Vec::new(), // Empty for now - could track oldest unacked sequence
        };
                
                let mut serializer = serializer.lock().await;
                if let Ok((data, _)) = serializer.serialize(&ping_packet, false) {
                    let _ = packet_tx.send((data, address));
                    debug!("Sent ping {} to {}", ping_id, address);
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
        
        // Emit packet received event - this will trigger the handlers
        let _ = self.event_tx.send(CircuitEvent::PacketReceived { packet });
        Ok(())
    }
}