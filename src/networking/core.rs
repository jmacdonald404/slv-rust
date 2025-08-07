//! Core networking controller for managing circuits and connections
//! 
//! The Core manages multiple circuits to different simulators and coordinates
//! the overall networking state of the client.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::circuit::{Circuit, CircuitEvent, CircuitOptions};
use crate::networking::handlers::{HandlerContext, PacketHandlerRegistry, PacketProcessor};
use crate::networking::packets::PacketWrapper;
use crate::networking::transport::{TransportConfig, UdpTransport};
use crate::networking::quic_transport::{QuicTransport, QuicTransportConfig};
use bytes::Bytes;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Core networking state
#[derive(Debug, Clone, PartialEq)]
pub enum CoreState {
    Idle,
    Connecting,
    Connected,
    Disconnecting,
    Disconnected,
}

/// Transport type following ADR-0002
pub enum TransportType {
    Quic(Arc<QuicTransport>),
    Udp(Arc<UdpTransport>),
}

impl TransportType {
    pub fn get_sender(&self) -> mpsc::UnboundedSender<(Bytes, SocketAddr)> {
        match self {
            TransportType::Quic(transport) => transport.get_sender(),
            TransportType::Udp(transport) => transport.get_sender(),
        }
    }
    
    pub async fn start(&self) -> NetworkResult<()> {
        match self {
            TransportType::Quic(transport) => transport.start().await,
            TransportType::Udp(transport) => transport.start().await,
        }
    }
    
    pub async fn set_packet_callback<F>(&self, callback: F) 
    where 
        F: Fn(PacketWrapper, SocketAddr) + Send + Sync + Clone + 'static
    {
        match self {
            TransportType::Quic(transport) => transport.set_packet_callback(callback).await,
            TransportType::Udp(transport) => transport.set_packet_callback(callback).await,
        }
    }
    
    /// Get the local UDP listen port
    pub fn local_addr(&self) -> SocketAddr {
        match self {
            TransportType::Quic(transport) => transport.local_addr(),
            TransportType::Udp(transport) => transport.local_addr(),
        }
    }
}

/// Core networking controller following ADR-0002 transport selection
pub struct Core {
    /// Current state
    state: Arc<RwLock<CoreState>>,
    
    /// Primary transport (QUIC or UDP fallback)
    transport: Arc<TransportType>,
    
    /// Active circuits (address -> circuit)
    circuits: Arc<RwLock<HashMap<SocketAddr, Arc<Circuit>>>>,
    
    /// Current primary circuit
    primary_circuit: Arc<RwLock<Option<Arc<Circuit>>>>,
    
    /// Packet handler registry
    handler_registry: Arc<PacketHandlerRegistry>,
    
    /// Packet processor
    packet_processor: Arc<PacketProcessor>,
    
    /// Event channels
    event_tx: mpsc::UnboundedSender<CoreEvent>,
    
    /// Packet processing channel
    packet_processing_tx: mpsc::UnboundedSender<(PacketWrapper, HandlerContext)>,
}

/// Events emitted by the core
#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// Core state changed
    StateChanged { old_state: CoreState, new_state: CoreState },
    /// Circuit connected
    CircuitConnected { address: SocketAddr },
    /// Circuit disconnected
    CircuitDisconnected { address: SocketAddr, reason: String },
    /// Error occurred
    Error { error: NetworkError },
}

impl Core {
    /// Create a new core with QUIC transport (ADR-0002)
    pub async fn new_with_quic(quic_config: QuicTransportConfig, udp_config: TransportConfig) -> NetworkResult<Self> {
        info!("🔐 Creating core with QUIC transport following ADR-0002");
        
        // Try to create QUIC transport first
        let transport = match QuicTransport::new(quic_config).await {
            Ok(quic_transport) => {
                info!("✅ QUIC transport initialized successfully");
                Arc::new(TransportType::Quic(Arc::new(quic_transport)))
            }
            Err(e) => {
                warn!("⚠️ QUIC transport failed, falling back to UDP: {}", e);
                let udp_transport = UdpTransport::new(udp_config).await?;
                Arc::new(TransportType::Udp(Arc::new(udp_transport)))
            }
        };
        
        Self::new_with_transport(transport).await
    }
    
    /// Create a new core with UDP transport (fallback)
    pub async fn new(transport_config: TransportConfig) -> NetworkResult<Self> {
        info!("Creating core with UDP transport (fallback mode)");
        let transport = Arc::new(UdpTransport::new(transport_config).await?);
        let transport_type = Arc::new(TransportType::Udp(transport));
        Self::new_with_transport(transport_type).await
    }
    
    /// Create core with specified transport type
    async fn new_with_transport(transport: Arc<TransportType>) -> NetworkResult<Self> {
        // Create handler registry and initialize default handlers
        let handler_registry = Arc::new(PacketHandlerRegistry::new());
        handler_registry.init_default_handlers().await;
        
        // Create packet processor
        let packet_processor = Arc::new(PacketProcessor::new(Arc::clone(&handler_registry)));
        
        // Create event channels
        let (event_tx, _) = mpsc::unbounded_channel();
        let (packet_processing_tx, packet_processing_rx) = mpsc::unbounded_channel();
        
        // Start packet processing
        let processor = Arc::clone(&packet_processor);
        tokio::spawn(async move {
            processor.start_processing(packet_processing_rx).await;
        });
        
        Ok(Self {
            state: Arc::new(RwLock::new(CoreState::Idle)),
            transport,
            circuits: Arc::new(RwLock::new(HashMap::new())),
            primary_circuit: Arc::new(RwLock::new(None)),
            handler_registry,
            packet_processor,
            event_tx,
            packet_processing_tx,
        })
    }
    
    /// Get current state
    pub async fn state(&self) -> CoreState {
        self.state.read().await.clone()
    }
    
    /// Get event receiver
    pub fn get_event_receiver(&self) -> mpsc::UnboundedReceiver<CoreEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        // In a real implementation, you'd manage multiple event receivers
        rx
    }
    
    /// Start the core
    pub async fn start(&self) -> NetworkResult<()> {
        self.set_state(CoreState::Connecting).await;
        
        // Start transport
        self.transport.start().await?;
        
        // Start background tasks
        self.start_packet_receiver().await;
        
        self.set_state(CoreState::Connected).await;
        info!("Core networking started");
        
        Ok(())
    }
    
    /// Connect to a simulator
    pub async fn connect_circuit(&self, options: CircuitOptions, handshake_tx: mpsc::Sender<crate::networking::client::HandshakeEvent>) -> NetworkResult<Arc<Circuit>> {
        info!("Connecting to circuit at {} with code {}", options.address, options.circuit_code);
        
        // Create channels for this circuit
        let packet_sender = self.transport.get_sender();
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        // Create circuit based on transport type (ADR-0002)
        let circuit = match &*self.transport {
            TransportType::Quic(quic_transport) => {
                Arc::new(Circuit::new_with_quic(options.clone(), Arc::clone(quic_transport), packet_rx, event_tx))
            }
            TransportType::Udp(_) => {
                Arc::new(Circuit::new_with_udp(options.clone(), packet_sender, packet_rx, event_tx))
            }
        };
        
        // Store circuit
        {
            let mut circuits = self.circuits.write().await;
            circuits.insert(options.address, Arc::clone(&circuit));
        }
        
        // Set as primary if we don't have one
        {
            let mut primary = self.primary_circuit.write().await;
            if primary.is_none() {
                *primary = Some(Arc::clone(&circuit));
            }
        }
        
        // Start circuit event handler
        let circuit_clone = Arc::clone(&circuit);
        let packet_processing_tx = self.packet_processing_tx.clone();
        let agent_id = options.agent_id;
        let session_id = options.session_id;
        let core_event_tx = self.event_tx.clone();
        let address = options.address;
        
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    CircuitEvent::Connected => {
                        info!("Circuit connected to {}", address);
                        let _ = core_event_tx.send(CoreEvent::CircuitConnected { address });
                    }
                    CircuitEvent::Disconnected { reason } => {
                        info!("Circuit disconnected from {}: {}", address, reason);
                        let _ = core_event_tx.send(CoreEvent::CircuitDisconnected { address, reason });
                        break;
                    }
                    CircuitEvent::PacketReceived { packet } => {
                        // First, let circuit handle ACK processing
                        if let Err(e) = circuit_clone.handle_incoming_packet(&packet.data).await {
                            error!("Failed to handle incoming packet for ACK processing: {}", e);
                        }
                        
                        // Then forward packet to processor for business logic
                        let context = HandlerContext {
                            circuit: Arc::clone(&circuit_clone),
                            agent_id,
                            session_id,
                            handshake_tx: handshake_tx.clone(),
                        };
                        
                        if let Err(_) = packet_processing_tx.send((packet, context)) {
                            // Processor has stopped
                            break;
                        }
                    }
                    CircuitEvent::Error { error } => {
                        error!("Circuit error on {}: {}", address, error);
                        let _ = core_event_tx.send(CoreEvent::Error { error });
                    }
                }
            }
        });
        
        // Start the circuit
        circuit.start().await?;
        
        info!("Circuit established to {}", options.address);
        Ok(circuit)
    }
    
    /// Get primary circuit
    pub async fn primary_circuit(&self) -> Option<Arc<Circuit>> {
        self.primary_circuit.read().await.clone()
    }
    
    /// Get circuit by address
    pub async fn get_circuit(&self, address: SocketAddr) -> Option<Arc<Circuit>> {
        self.circuits.read().await.get(&address).cloned()
    }
    
    /// Disconnect from a simulator
    pub async fn disconnect_circuit(&self, address: SocketAddr) -> NetworkResult<()> {
        let circuit = {
            let mut circuits = self.circuits.write().await;
            circuits.remove(&address)
        };
        
        if let Some(circuit) = circuit {
            circuit.stop().await?;
            
            // Clear primary circuit if this was it
            {
                let mut primary = self.primary_circuit.write().await;
                if let Some(ref primary_circuit) = *primary {
                    if primary_circuit.address() == address {
                        *primary = None;
                    }
                }
            }
            
            info!("Disconnected circuit from {}", address);
        }
        
        Ok(())
    }
    
    /// Shutdown the core
    pub async fn shutdown(&self) -> NetworkResult<()> {
        self.set_state(CoreState::Disconnecting).await;
        
        // Disconnect all circuits
        let addresses: Vec<SocketAddr> = {
            self.circuits.read().await.keys().cloned().collect()
        };
        
        for address in addresses {
            if let Err(e) = self.disconnect_circuit(address).await {
                warn!("Error disconnecting circuit {}: {}", address, e);
            }
        }
        
        self.set_state(CoreState::Disconnected).await;
        info!("Core networking shutdown complete");
        
        Ok(())
    }
    
    /// Start packet receiver task
    async fn start_packet_receiver(&self) {
        let circuits = Arc::clone(&self.circuits);
        
        // Set up the packet callback (like homunculus socket.receive)
        self.transport.set_packet_callback(move |packet, src_addr| {
            let circuits_clone = Arc::clone(&circuits);
            tokio::spawn(async move {
                // Find the circuit this packet belongs to by source address
                let circuits_guard = circuits_clone.read().await;
                if let Some(circuit) = circuits_guard.get(&src_addr) {
                    // Forward packet to the appropriate circuit
                    info!("Received packet from {} for circuit", src_addr);
                    if let Err(e) = circuit.inject_packet(packet).await {
                        warn!("Failed to inject packet into circuit {}: {}", src_addr, e);
                    }
                } else {
                    info!("Received packet from unknown address: {}", src_addr);
                }
            });
        }).await;
    }
    
    /// Set core state and emit event
    async fn set_state(&self, new_state: CoreState) {
        let old_state = {
            let mut state = self.state.write().await;
            let old = state.clone();
            *state = new_state.clone();
            old
        };
        
        if old_state != new_state {
            let _ = self.event_tx.send(CoreEvent::StateChanged { old_state, new_state });
        }
    }
    
    /// Get handler registry (for adding custom handlers)
    pub fn handler_registry(&self) -> Arc<PacketHandlerRegistry> {
        Arc::clone(&self.handler_registry)
    }
    
    /// Get the local UDP listen address
    pub fn local_addr(&self) -> SocketAddr {
        self.transport.local_addr()
    }
}