//! High-level Second Life client interface
//! 
//! This module provides the main client API that applications use to connect
//! to Second Life simulators and interact with the virtual world.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::circuit::CircuitOptions;
use crate::networking::core::{Core, CoreEvent, CoreState};
use crate::networking::packets::generated::*;
use crate::networking::transport::TransportConfig;
use crate::networking::auth::SessionInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};

#[derive(Debug)]
pub enum HandshakeEvent {
    RegionHandshake,
    AgentMovementComplete,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventQueueGetEvent {
    pub message: String,
    pub body: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventQueueGetResponse {
    pub events: Vec<EventQueueGetEvent>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectUpdateEventBody {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "P")]
    pub position: [f32; 3],
}

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
    /// Session information
    pub session_info: Option<SessionInfo>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            transport: TransportConfig::default(),
            agent_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            default_timeout: std::time::Duration::from_secs(30),
            session_info: None,
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
    
    /// Handshake completion channels
    handshake_tx: mpsc::Sender<HandshakeEvent>,
    handshake_rx: Arc<RwLock<mpsc::Receiver<HandshakeEvent>>>,
    
    /// Background task handles
    _background_tasks: Vec<tokio::task::JoinHandle<()>>,

    /// Session information
    session_info: SessionInfo,

    /// HTTP client for EventQueueGet
    http_client: reqwest::Client,
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
    /// Region handshake received
    RegionHandshakeReceived,
    /// Agent movement complete received
    AgentMovementCompleteReceived,
    /// Error occurred
    Error { error: NetworkError },
}

impl Client {
    /// Create a new client
    pub async fn new(config: ClientConfig, session_info: SessionInfo) -> NetworkResult<Self> {
        // Create networking core
        let core = Arc::new(Core::new(config.transport.clone()).await?);
        
        // Create event channels
        let (event_tx, _) = mpsc::unbounded_channel();
        let (handshake_tx, handshake_rx) = mpsc::channel(100);
        
        let client = Self {
            config,
            core,
            state: Arc::new(RwLock::new(ClientState::Disconnected)),
            event_tx,
            handshake_tx,
            handshake_rx: Arc::new(RwLock::new(handshake_rx)),
            _background_tasks: Vec::new(),
            session_info,
            http_client: reqwest::Client::new(),
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
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
        };
        
        // Connect to the circuit
        let circuit = self.core.connect_circuit(options, self.handshake_tx.clone()).await?;
        
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
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
            message: crate::networking::packets::types::LLVariable2::from_string(message),
            r#type: 1, // Normal chat
            channel: channel as i32,
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
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
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
        };
        
        // AgentUpdate is sent unreliably
        circuit.send(&agent_update).await?;
        
        Ok(())
    }
    
    /// Perform initial handshake with simulator following homunculus protocol
    async fn perform_handshake(&self, circuit: &crate::networking::circuit::Circuit) -> NetworkResult<()> {
        info!("Starting handshake with simulator following homunculus protocol");
        
        // Step 1: Send UseCircuitCode - establishes the circuit
        let use_circuit_code = UseCircuitCode {
            code: circuit.circuit_code(),
            session_id: self.session_info.session_id,
            id: self.session_info.agent_id.as_bytes().to_vec(),
        };
        
        circuit.send_reliable(&use_circuit_code, self.config.default_timeout).await?;
        info!("Sent UseCircuitCode");
        
        // Step 2: Send CompleteAgentMovement - critical for main region handshake
        // This triggers the server to send RegionHandshake AND AgentMovementComplete
        let complete_agent_movement = CompleteAgentMovement {
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
            circuit_code: circuit.circuit_code(),
        };
        
        circuit.send_reliable(&complete_agent_movement, self.config.default_timeout).await?;
        info!("Sent CompleteAgentMovement");
        
        // Step 3: Send UUIDNameRequest for essential avatar data
        let uuid_name_request = UUIDNameRequest {
            uuidname_block: vec![UUIDNameBlockBlock {
                id: self.session_info.agent_id,
            }],
        };
        
        circuit.send_reliable(&uuid_name_request, self.config.default_timeout).await?;
        info!("Sent UUIDNameRequest");
        
        info!("Initial handshake packets sent");
        info!("Server will respond with:");
        info!("  1. RegionHandshake (handled by RegionHandshakeHandler)");
        info!("  2. AgentMovementComplete (handled by AgentMovementCompleteHandler)");
        info!("The AgentMovementCompleteHandler will complete the full handshake sequence");
        
        // Step 4: Start EventQueueGet
        self.start_event_queue_get().await?;

        // Step 5: Wait for RegionHandshake and AgentMovementComplete
        let mut region_handshake_received = false;
        let mut agent_movement_complete_received = false;

        let mut handshake_rx = self.handshake_rx.write().await;

        while !region_handshake_received || !agent_movement_complete_received {
            match timeout(Duration::from_secs(30), handshake_rx.recv()).await {
                Ok(Some(event)) => {
                    match event {
                        HandshakeEvent::RegionHandshake => {
                            info!("RegionHandshake received");
                            region_handshake_received = true;
                        },
                        HandshakeEvent::AgentMovementComplete => {
                            info!("AgentMovementComplete received");
                            agent_movement_complete_received = true;
                        },
                    }
                },
                Ok(None) => {
                    return Err(NetworkError::HandshakeFailed { reason: "Handshake channel closed unexpectedly".to_string() });
                },
                Err(_) => {
                    return Err(NetworkError::HandshakeFailed { reason: "Handshake timed out".to_string() });
                }
            }
        }

        info!("Handshake complete!");
        Ok(())
    }

    /// Start the EventQueueGet long-polling connection
    async fn start_event_queue_get(&self) -> NetworkResult<()> {
        let eqg_url = self.session_info.capabilities.as_ref()
            .and_then(|caps| caps.get("EventQueueGet"))
            .ok_or_else(|| NetworkError::Other { reason: "EventQueueGet capability not found".to_string() })?
            .clone();

        info!("Starting EventQueueGet connection to {}", eqg_url);

        let client = self.http_client.clone();
        let session_id = self.session_info.session_id;
        let agent_id = self.session_info.agent_id;
        let eqg_url = eqg_url.clone();

        tokio::spawn(async move {
            loop {
                debug!("Sending EventQueueGet request");
                let request_body = format!(
                    r#"{{"session_id": "{}", "agent_id": "{}", "ack": []}}"#, 
                    session_id, agent_id
                );
                
                match client.post(eqg_url.clone())
                    .header("Content-Type", "application/json")
                    .body(request_body)
                    .send()
                    .await {
                    Ok(response) => {
                        if response.status().is_success() {
                            match response.text().await {
                                Ok(text) => {
                                    debug!("EventQueueGet response: {}", text);
                                    match serde_json::from_str::<EventQueueGetResponse>(&text) {
                                        Ok(eqg_response) => {
                                            for event in eqg_response.events {
                                                debug!("Received EventQueueGet event: {:?}", event);
                                                match event.message.as_str() {
                                                    "ObjectUpdate" => {
                                                        match serde_json::from_value::<ObjectUpdateEventBody>(event.body) {
                                                            Ok(object_update) => {
                                                                info!("ObjectUpdate received for ID: {}, Position: {:?}", object_update.id, object_update.position);
                                                                // Here you would typically update the avatar's position in your world state
                                                                // and potentially send an AgentUpdate if the avatar's position needs correction.
                                                            },
                                                            Err(e) => {
                                                                tracing::error!("Failed to deserialize ObjectUpdate event body: {}", e);
                                                            }
                                                        }
                                                    },
                                                    _ => {
                                                        debug!("Unhandled EventQueueGet event type: {}", event.message);
                                                    }
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            tracing::error!("Failed to deserialize EventQueueGet response: {}", e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    tracing::error!("Failed to read EventQueueGet response text: {}", e);
                                }
                            }
                        } else {
                            tracing::error!("EventQueueGet request failed with status: {}", response.status());
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to send EventQueueGet request: {}", e);
                    }
                }
                // Sleep for a bit before sending the next request (long-polling)
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

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