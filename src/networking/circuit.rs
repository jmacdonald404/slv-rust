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
    
    /// Get packets that need retransmission
    fn get_retransmits(&mut self, retry_timeout: Duration, max_retries: u8) -> Vec<(u32, PacketWrapper)> {
        let now = Instant::now();
        let mut retransmits = Vec::new();
        let mut to_remove = Vec::new();
        
        for (sequence, pending) in &mut self.pending_reliable {
            if now.duration_since(pending.sent_at) >= retry_timeout {
                if pending.retry_count >= max_retries {
                    // Too many retries - give up
                    to_remove.push(*sequence);
                    if let Some(tx) = pending.resolve_tx.take() {
                        let _ = tx.send(()); // Resolve with failure
                    }
                } else {
                    // Retry
                    pending.retry_count += 1;
                    pending.sent_at = now;
                    // Create a new PacketWrapper for retransmission (avoid cloning PendingPacket)
                    let wrapper = PacketWrapper {
                        data: pending.packet.data.clone(),
                        reliable: pending.packet.reliable,
                        sequence: pending.packet.sequence,
                        packet_id: pending.packet.packet_id,
                        frequency: pending.packet.frequency,
                    };
                    retransmits.push((*sequence, wrapper));
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
    Connecting,
    Connected,
    Disconnecting,
    Disconnected,
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
    
    /// Channel for sending packets to transport
    packet_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
    
    /// Channel for receiving packets from transport
    packet_rx: Arc<Mutex<mpsc::UnboundedReceiver<PacketWrapper>>>,
    
    /// Channel for outgoing events
    event_tx: mpsc::UnboundedSender<CircuitEvent>,
    
    /// Retry configuration
    retry_timeout: Duration,
    max_retries: u8,
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
            packet_tx,
            packet_rx: Arc::new(Mutex::new(packet_rx)),
            event_tx,
            retry_timeout: Duration::from_secs(3),
            max_retries: 3,
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
    pub async fn send_reliable<P: Packet>(&self, packet: &P, timeout_duration: Duration) -> NetworkResult<()> {
        let (resolve_tx, resolve_rx) = tokio::sync::oneshot::channel();
        
        let mut serializer = self.serializer.lock().await;
        let (data, sequence) = serializer.serialize(packet, true)?;
        
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
        
        // Wait for acknowledgment with timeout
        match timeout(timeout_duration, resolve_rx).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(NetworkError::HandshakeTimeout),
            Err(_) => Err(NetworkError::HandshakeTimeout),
        }
    }
    
    /// Start the circuit (begin packet processing)
    pub async fn start(&self) -> NetworkResult<()> {
        {
            let mut state = self.state.write().await;
            *state = CircuitState::Connected;
        }
        
        // Start background tasks
        self.start_packet_processor().await;
        self.start_retry_handler().await;
        self.start_ack_sender().await;
        
        // Emit connected event
        let _ = self.event_tx.send(CircuitEvent::Connected);
        
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
                agent_data: AgentDataBlock {
                    agent_id: self.options.agent_id,
                    session_id: self.options.session_id,
                    circuit_code: self.options.circuit_code,
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
    
    /// Start packet processing task
    async fn start_packet_processor(&self) {
        let packet_rx = Arc::clone(&self.packet_rx);
        let deserializer = Arc::clone(&self.deserializer);
        let acknowledger = Arc::clone(&self.acknowledger);
        let event_tx = self.event_tx.clone();
        let state = Arc::clone(&self.state);
        
        tokio::spawn(async move {
            let mut rx = packet_rx.lock().await;
            
            while let Some(wrapper) = rx.recv().await {
                // Check if circuit is still active
                if *state.read().await == CircuitState::Disconnected {
                    break;
                }
                
                // Handle reliable packet acknowledgment
                if wrapper.reliable {
                    let mut ack = acknowledger.lock().await;
                    if ack.is_sequence_new(wrapper.sequence) {
                        ack.queue_ack(wrapper.sequence);
                    } else {
                        // Duplicate packet - ignore
                        continue;
                    }
                }
                
                // Emit packet received event
                let _ = event_tx.send(CircuitEvent::PacketReceived { packet: wrapper });
            }
        });
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
                    let serializer = serializer.lock().await;
                    if let Ok(data) = serializer.serialize_wrapper(&wrapper) {
                        let _ = packet_tx.send((data, address));
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
                            .map(|id| PacketAckPacketsBlock { id })
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
        for packet_block in &ack_packet.packets {
            ack.handle_ack(packet_block.id);
        }
    }
}