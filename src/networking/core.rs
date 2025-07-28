//! Core networking controller for managing circuits and connections
//! 
//! The Core manages multiple circuits to different simulators and coordinates
//! the overall networking state of the client.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::circuit::{Circuit, CircuitEvent, CircuitOptions};
use crate::networking::handlers::{HandlerContext, PacketHandlerRegistry, PacketProcessor};
use crate::networking::packets::PacketWrapper;
use crate::networking::transport::{TransportConfig, UdpTransport};
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

/// Core networking controller
pub struct Core {
    /// Current state
    state: Arc<RwLock<CoreState>>,
    
    /// UDP transport
    transport: Arc<UdpTransport>,
    
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
    /// Create a new core
    pub async fn new(transport_config: TransportConfig) -> NetworkResult<Self> {
        // Create transport
        let transport = Arc::new(UdpTransport::new(transport_config).await?);
        
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
    pub async fn connect_circuit(&self, options: CircuitOptions) -> NetworkResult<Arc<Circuit>> {
        debug!("Connecting to circuit at {} with code {}", options.address, options.circuit_code);
        
        // Create channels for this circuit
        let packet_sender = self.transport.get_sender();
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        // Create circuit
        let circuit = Arc::new(Circuit::new(options.clone(), packet_sender, packet_rx, event_tx));
        
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
                        // Forward packet to processor
                        let context = HandlerContext {
                            circuit: Arc::clone(&circuit_clone),
                            agent_id,
                            session_id,
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
        
        debug!("Circuit established to {}", options.address);
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
            
            debug!("Disconnected circuit from {}", address);
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
        let mut packet_rx = self.transport.get_receiver();
        let circuits = Arc::clone(&self.circuits);
        
        tokio::spawn(async move {
            while let Some(packet) = packet_rx.recv().await {
                // Find the circuit this packet belongs to
                // For now, we'll use a simple approach - in a real implementation,
                // you'd need to track which packets came from which addresses
                
                let circuits_guard = circuits.read().await;
                if let Some(circuit) = circuits_guard.values().next() {
                    // Forward packet to the first available circuit
                    // This is simplified - you'd want proper routing
                    if let Some(mut circuit_rx) = None::<mpsc::UnboundedReceiver<PacketWrapper>> {
                        // This is where you'd forward the packet to the circuit
                        // The actual implementation would require a different architecture
                    }
                }
            }
        });
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
}