//! High-level Second Life client interface
//! 
//! This module provides the main client API that applications use to connect
//! to Second Life simulators and interact with the virtual world.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::circuit::CircuitOptions;
use crate::networking::core::{Core, CoreEvent, CoreState};
use crate::networking::packets::generated::*;
use crate::networking::transport::TransportConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};
use uuid::Uuid;

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Transport configuration
    pub transport: TransportConfig,
    /// Agent information
    pub agent_id: Uuid,
    pub session_id: Uuid,
    /// Default timeout for operations
    pub default_timeout: std::time::Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            transport: TransportConfig::default(),
            agent_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            default_timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// High-level Second Life client
pub struct Client {
    /// Client configuration
    config: ClientConfig,
    
    /// Networking core
    core: Arc<Core>,
    
    /// Current state
    state: Arc<RwLock<ClientState>>,
    
    /// Event channels
    event_tx: mpsc::UnboundedSender<ClientEvent>,
    
    /// Background task handles
    _background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// Client state
#[derive(Debug, Clone, PartialEq)]
pub enum ClientState {
    Disconnected,
    Connecting,
    Connected,
    LoggingIn,
    LoggedIn,
    Disconnecting,
}

/// Events emitted by the client
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// Client state changed
    StateChanged { old_state: ClientState, new_state: ClientState },
    /// Connected to simulator
    Connected { address: SocketAddr },
    /// Disconnected from simulator
    Disconnected { address: SocketAddr, reason: String },
    /// Login completed successfully
    LoginComplete,
    /// Error occurred
    Error { error: NetworkError },
}

impl Client {
    /// Create a new client
    pub async fn new(config: ClientConfig) -> NetworkResult<Self> {
        // Create networking core
        let core = Arc::new(Core::new(config.transport.clone()).await?);
        
        // Create event channels
        let (event_tx, _) = mpsc::unbounded_channel();
        
        let client = Self {
            config,
            core,
            state: Arc::new(RwLock::new(ClientState::Disconnected)),
            event_tx,
            _background_tasks: Vec::new(),
        };
        
        // Start background tasks
        client.start_background_tasks().await;
        
        Ok(client)
    }
    
    /// Get current state
    pub async fn state(&self) -> ClientState {
        self.state.read().await.clone()
    }
    
    /// Get event receiver
    pub fn get_event_receiver(&self) -> mpsc::UnboundedReceiver<ClientEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        // In a real implementation, you'd manage multiple event receivers
        rx
    }
    
    /// Connect to a simulator
    pub async fn connect(&self, simulator_address: SocketAddr, circuit_code: u32) -> NetworkResult<()> {
        info!("Connecting to simulator at {} with circuit code {}", simulator_address, circuit_code);
        
        self.set_state(ClientState::Connecting).await;
        
        // Start the core
        self.core.start().await?;
        
        // Create circuit options
        let options = CircuitOptions {
            circuit_code,
            address: simulator_address,
            agent_id: self.config.agent_id,
            session_id: self.config.session_id,
        };
        
        // Connect to the circuit
        let circuit = self.core.connect_circuit(options).await?;
        
        self.set_state(ClientState::LoggingIn).await;
        
        // Perform handshake
        self.perform_handshake(&circuit).await?;
        
        self.set_state(ClientState::LoggedIn).await;
        let _ = self.event_tx.send(ClientEvent::LoginComplete);
        
        info!("Successfully connected and logged in to {}", simulator_address);
        Ok(())
    }
    
    /// Disconnect from current simulator
    pub async fn disconnect(&self) -> NetworkResult<()> {
        self.set_state(ClientState::Disconnecting).await;
        
        // Shutdown the core (this will disconnect all circuits)
        self.core.shutdown().await?;
        
        self.set_state(ClientState::Disconnected).await;
        info!("Disconnected from simulator");
        
        Ok(())
    }
    
    /// Send a chat message
    pub async fn send_chat(&self, message: &str, channel: u32) -> NetworkResult<()> {
        let circuit = self.core.primary_circuit().await
            .ok_or(NetworkError::CircuitNotFound { id: 0 })?;
        
        // Create ChatFromViewer packet
        let chat_packet = ChatFromViewer {
            agent_data: AgentDataBlock {
                agent_id: self.config.agent_id,
                session_id: self.config.session_id,
                circuit_code: circuit.circuit_code(),
            },
            chat_data: ChatDataBlock {
                message: crate::networking::packets::types::LLVariable1::from_string(message),
                chat_type: 1, // Normal chat
                channel,
            },
        };
        
        circuit.send(&chat_packet).await?;
        debug!("Sent chat message: {}", message);
        
        Ok(())
    }
    
    /// Update agent position and orientation
    pub async fn update_agent(&self, 
                             position: Option<crate::networking::packets::types::LLVector3>,
                             rotation: Option<crate::networking::packets::types::LLQuaternion>) -> NetworkResult<()> {
        let circuit = self.core.primary_circuit().await
            .ok_or(NetworkError::CircuitNotFound { id: 0 })?;
        
        let agent_update = AgentUpdate {
            agent_data: AgentUpdateDataBlock {
                agent_id: self.config.agent_id,
                session_id: self.config.session_id,
                body_rotation: rotation.unwrap_or_else(|| 
                    crate::networking::packets::types::LLQuaternion::identity()),
                head_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                state: 0,
                camera_center: position.unwrap_or_else(|| 
                    crate::networking::packets::types::LLVector3::new(128.0, 128.0, 25.0)),
                camera_at_axis: crate::networking::packets::types::LLVector3::new(1.0, 0.0, 0.0),
                camera_left_axis: crate::networking::packets::types::LLVector3::new(0.0, 1.0, 0.0),
                camera_up_axis: crate::networking::packets::types::LLVector3::new(0.0, 0.0, 1.0),
                far: 256.0,
                control_flags: 0,
                flags: 0,
            },
        };
        
        // AgentUpdate is sent unreliably
        circuit.send(&agent_update).await?;
        
        Ok(())
    }
    
    /// Perform initial handshake with simulator
    async fn perform_handshake(&self, circuit: &crate::networking::circuit::Circuit) -> NetworkResult<()> {
        debug!("Starting handshake with simulator");
        
        // Send UseCircuitCode
        let use_circuit_code = UseCircuitCode {
            circuit_code: CircuitCodeBlock {
                code: circuit.circuit_code(),
                session_id: self.config.session_id,
                id: self.config.agent_id,
            },
        };
        
        circuit.send_reliable(&use_circuit_code, self.config.default_timeout).await?;
        debug!("Sent UseCircuitCode");
        
        // Send CompleteAgentMovement
        let complete_agent_movement = CompleteAgentMovement {
            agent_data: AgentDataBlock {
                agent_id: self.config.agent_id,
                session_id: self.config.session_id,
                circuit_code: circuit.circuit_code(),
            },
        };
        
        circuit.send_reliable(&complete_agent_movement, self.config.default_timeout).await?;
        debug!("Sent CompleteAgentMovement");
        
        // The rest of the handshake (RegionHandshakeReply, etc.) will be handled
        // by the packet handlers when the server sends RegionHandshake
        
        Ok(())
    }
    
    /// Start background tasks
    async fn start_background_tasks(&self) {
        let state = Arc::clone(&self.state);
        let event_tx = self.event_tx.clone();
        let mut core_events = self.core.get_event_receiver();
        
        // Core event handler
        tokio::spawn(async move {
            while let Some(event) = core_events.recv().await {
                match event {
                    CoreEvent::StateChanged { old_state, new_state } => {
                        debug!("Core state changed: {:?} -> {:?}", old_state, new_state);
                    }
                    CoreEvent::CircuitConnected { address } => {
                        let _ = event_tx.send(ClientEvent::Connected { address });
                    }
                    CoreEvent::CircuitDisconnected { address, reason } => {
                        let _ = event_tx.send(ClientEvent::Disconnected { address, reason });
                    }
                    CoreEvent::Error { error } => {
                        let _ = event_tx.send(ClientEvent::Error { error });
                    }
                }
            }
        });
    }
    
    /// Set client state and emit event
    async fn set_state(&self, new_state: ClientState) {
        let old_state = {
            let mut state = self.state.write().await;
            let old = state.clone();
            *state = new_state.clone();
            old
        };
        
        if old_state != new_state {
            let _ = self.event_tx.send(ClientEvent::StateChanged { old_state, new_state });
        }
    }
    
    /// Get the networking core (for advanced usage)
    pub fn core(&self) -> Arc<Core> {
        Arc::clone(&self.core)
    }
}