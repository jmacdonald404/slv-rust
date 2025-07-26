use crate::ui::UiState;
use crate::networking::commands::NetworkCommand;
use crate::world::*;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::sync::mpsc;

/*
// New: Main-thread-only RenderContext for wgpu/winit fields
pub struct RenderContext<'a> {
    pub surface: wgpu::Surface<'a>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub window: Arc<winit::window::Window>,
    pub ui_ctx: Option<crate::ui::UiContext>,
}
*/

/// Main application structure that manages communication between UI, networking, and world systems
pub struct App {
    // Network communication channels
    network_command_sender: mpsc::UnboundedSender<NetworkCommand>,
    
    // Event receivers from the network layer
    chat_event_receiver: mpsc::UnboundedReceiver<ChatEvent>,
    object_update_receiver: mpsc::UnboundedReceiver<ObjectUpdateEvent>,
    agent_movement_receiver: mpsc::UnboundedReceiver<AgentMovementCompleteEvent>,
    health_update_receiver: mpsc::UnboundedReceiver<HealthUpdateEvent>,
    avatar_update_receiver: mpsc::UnboundedReceiver<AvatarDataUpdateEvent>,
    region_handshake_receiver: mpsc::UnboundedReceiver<RegionHandshakeEvent>,
    connection_status_receiver: mpsc::UnboundedReceiver<ConnectionStatusEvent>,
    keep_alive_receiver: mpsc::UnboundedReceiver<KeepAliveEvent>,
    
    // Application state
    pub ui_state: UiState,
    pub chat_history: Vec<ChatEvent>,
    pub connection_status: ConnectionStatus,
    
    // World state
    pub objects: std::collections::HashMap<u32, ObjectUpdateEvent>,
}

impl App {
    /// Create a new App instance and return the channel endpoints for the network layer
    pub fn new(ui_state: UiState) -> (Self, AppNetworkChannels) {
        // Create network command channel
        let (network_command_sender, network_command_receiver) = mpsc::unbounded_channel();
        
        // Create event channels
        let (chat_sender, chat_event_receiver) = mpsc::unbounded_channel();
        let (object_update_sender, object_update_receiver) = mpsc::unbounded_channel();
        let (agent_movement_sender, agent_movement_receiver) = mpsc::unbounded_channel();
        let (health_update_sender, health_update_receiver) = mpsc::unbounded_channel();
        let (avatar_update_sender, avatar_update_receiver) = mpsc::unbounded_channel();
        let (region_handshake_sender, region_handshake_receiver) = mpsc::unbounded_channel();
        let (connection_status_sender, connection_status_receiver) = mpsc::unbounded_channel();
        let (keep_alive_sender, keep_alive_receiver) = mpsc::unbounded_channel();
        
        let app = Self {
            network_command_sender,
            chat_event_receiver,
            object_update_receiver,
            agent_movement_receiver,
            health_update_receiver,
            avatar_update_receiver,
            region_handshake_receiver,
            connection_status_receiver,
            keep_alive_receiver,
            ui_state,
            chat_history: Vec::new(),
            connection_status: ConnectionStatus::Disconnected,
            objects: std::collections::HashMap::new(),
        };
        
        let network_channels = AppNetworkChannels {
            command_receiver: network_command_receiver,
            chat_sender,
            object_update_sender,
            agent_movement_sender,
            health_update_sender,
            avatar_update_sender,
            region_handshake_sender,
            connection_status_sender,
            keep_alive_sender,
        };
        
        (app, network_channels)
    }
    
    /// Send a command to the network layer
    pub fn send_network_command(&self, command: NetworkCommand) {
        if let Err(e) = self.network_command_sender.send(command) {
            tracing::error!("Failed to send network command: {}", e);
        }
    }
    
    /// Send a chat message
    pub fn send_chat(&self, message: String) {
        self.send_network_command(NetworkCommand::local_chat(message));
    }
    
    /// Update agent position
    pub fn update_agent_position(&self, position: (f32, f32, f32), camera_at: (f32, f32, f32), camera_eye: (f32, f32, f32), controls: u32) {
        self.send_network_command(NetworkCommand::agent_update(position, camera_at, camera_eye, controls));
    }
    
    /// Process all pending events from the network layer
    pub fn process_events(&mut self) {
        // Process chat events
        while let Ok(event) = self.chat_event_receiver.try_recv() {
            tracing::info!("Chat: {}: {}", event.sender_name, event.message);
            self.chat_history.push(event);
        }
        
        // Process object updates
        while let Ok(event) = self.object_update_receiver.try_recv() {
            tracing::debug!("Object update: {} at {:?}", event.object_id, event.position);
            self.objects.insert(event.object_id, event);
        }
        
        // Process agent movement completion
        while let Ok(event) = self.agent_movement_receiver.try_recv() {
            tracing::info!("Agent movement complete: {}", event.agent_id);
        }
        
        // Process health updates
        while let Ok(event) = self.health_update_receiver.try_recv() {
            tracing::debug!("Health update: {}", event.health);
        }
        
        // Process avatar updates
        while let Ok(event) = self.avatar_update_receiver.try_recv() {
            tracing::debug!("Avatar update: {} {}", event.firstname, event.lastname);
        }
        
        // Process region handshake
        while let Ok(event) = self.region_handshake_receiver.try_recv() {
            tracing::info!("Region handshake: {}", event.region_name);
        }
        
        // Process connection status changes
        while let Ok(event) = self.connection_status_receiver.try_recv() {
            tracing::info!("Connection status: {:?}", event.status);
            self.connection_status = event.status;
        }
        
        // Process keep alive events
        while let Ok(_event) = self.keep_alive_receiver.try_recv() {
            // Keep alive events are mostly for internal monitoring
            tracing::trace!("Keep alive received");
        }
    }
    
    /// Get recent chat messages
    pub fn get_recent_chat(&self, count: usize) -> &[ChatEvent] {
        let start = if self.chat_history.len() > count {
            self.chat_history.len() - count
        } else {
            0
        };
        &self.chat_history[start..]
    }
    
    /// Get current connection status
    pub fn get_connection_status(&self) -> &ConnectionStatus {
        &self.connection_status
    }
}

/// Channel endpoints for the network layer
pub struct AppNetworkChannels {
    pub command_receiver: mpsc::UnboundedReceiver<NetworkCommand>,
    pub chat_sender: mpsc::UnboundedSender<ChatEvent>,
    pub object_update_sender: mpsc::UnboundedSender<ObjectUpdateEvent>,
    pub agent_movement_sender: mpsc::UnboundedSender<AgentMovementCompleteEvent>,
    pub health_update_sender: mpsc::UnboundedSender<HealthUpdateEvent>,
    pub avatar_update_sender: mpsc::UnboundedSender<AvatarDataUpdateEvent>,
    pub region_handshake_sender: mpsc::UnboundedSender<RegionHandshakeEvent>,
    pub connection_status_sender: mpsc::UnboundedSender<ConnectionStatusEvent>,
    pub keep_alive_sender: mpsc::UnboundedSender<KeepAliveEvent>,
}

// Multithreaded shared state for UI/logic (keeping for backward compatibility)
pub struct AppState {
    pub ui_state: UiState,
    // Add other Send + Sync fields as needed
}

// impl AppState {
//     pub async fn cleanup(&mut self) {
//         let mut should_clear_udp_circuit = false;
//         if let Some(circuit_mutex) = self.ui_state.udp_circuit.as_mut() {
//             if let Some(session) = &self.ui_state.login_state.session_info {
//                 if let Ok(ip) = session.sim_ip.parse() {
//                     let sim_addr = SocketAddr::new(ip, session.sim_port);
//                     {
//                         let mut circuit = circuit_mutex.lock().await;
//                         circuit.disconnect_and_logout(&sim_addr).await;
//                     }
//                     should_clear_udp_circuit = true;
//                     self.ui_state.udp_progress = crate::ui::UdpConnectionProgress::NotStarted;
//                     self.ui_state.login_state.status_message = "Disconnected from server.".to_string();
//                 }
//             }
//         }
//         if should_clear_udp_circuit {
//             self.ui_state.udp_circuit = None;
//         }
//     }
// }
