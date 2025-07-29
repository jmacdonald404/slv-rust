//! Example of the improved networking architecture
//! 
//! This example demonstrates how to use the new event-driven,
//! maintainable networking system inspired by homunculus.

use crate::networking::{
    manager::{NetworkManager, NetworkEvent, NetworkStatus},
    auth::{LoginCredentials, Grid},
    handlers::system::{HandlerRegistry, TypedPacketHandler, HandlerContext},
    packets::generated::RegionHandshake,
};
use crate::networking::NetworkResult;
use async_trait::async_trait;
use tracing::{info, debug, error};

/// Example packet handler for RegionHandshake
#[derive(Debug, Clone)]
struct MyRegionHandshakeHandler;

#[async_trait]
impl TypedPacketHandler<RegionHandshake> for MyRegionHandshakeHandler {
    async fn handle_typed(&self, packet: &RegionHandshake, context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Received RegionHandshake - Welcome to the region!");
        
        // Extract region info from packet
        debug!("Region info: {:?}", packet);
        
        // Send RegionHandshakeReply
        use crate::networking::packets::generated::RegionHandshakeReply;
        let reply = RegionHandshakeReply {
            region_info: crate::networking::packets::generated::RegionInfoBlock {
                flags: 0x1 | 0x2, // VOCACHE_CULLING_ENABLED | VOCACHE_IS_EMPTY
            },
        };
        
        context.circuit.send(&reply).await?;
        info!("✅ Sent RegionHandshakeReply");
        
        Ok(())
    }
    
    fn priority(&self) -> i32 {
        100 // High priority for important handshake
    }
}

/// Example client using the improved architecture
pub struct ImprovedClient {
    network_manager: NetworkManager,
    handler_registry: HandlerRegistry,
}

impl ImprovedClient {
    /// Create a new improved client
    pub fn new() -> Self {
        Self {
            network_manager: NetworkManager::new(1000), // Event channel capacity
            handler_registry: HandlerRegistry::new(),
        }
    }
    
    /// Initialize the client with packet handlers
    pub async fn initialize(&self) -> NetworkResult<()> {
        info!("🚀 Initializing improved client...");
        
        // Register packet handlers
        self.handler_registry
            .register_handler::<RegionHandshake, _>(MyRegionHandshakeHandler)
            .await;
        
        // Register more handlers using the macro
        let chat_handler = crate::packet_handler!(
            crate::networking::packets::generated::ChatFromSimulator,
            50, // Medium priority
            |packet, context| {
                info!("💬 Chat: {}", packet.chat_data.message);
                Ok(())
            }
        );
        
        self.handler_registry
            .register_handler::<crate::networking::packets::generated::ChatFromSimulator, _>(chat_handler)
            .await;
        
        let stats = self.handler_registry.get_stats().await;
        info!("📊 Registered {} handlers for {} packet types", 
              stats.packet_handlers, stats.packet_types);
        
        Ok(())
    }
    
    /// Connect to Second Life
    pub async fn connect(&mut self, username: String, password: String) -> NetworkResult<()> {
        info!("🔑 Connecting as {}...", username);
        
        // Create login credentials
        let credentials = LoginCredentials::new(username, password)
            .with_grid(Grid::SecondLifeMain)
            .with_start_location("last".to_string());
        
        // Start network event monitoring
        let mut events = self.network_manager.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                match event {
                    NetworkEvent::StatusChanged { old, new } => {
                        info!("📡 Network status: {:?} → {:?}", old, new);
                    }
                    NetworkEvent::Connected { session } => {
                        info!("🎉 Connected! Welcome, {} {}!", 
                              session.first_name, session.last_name);
                    }
                    NetworkEvent::CircuitConnected { address } => {
                        debug!("🔌 Circuit connected: {}", address);
                    }
                    NetworkEvent::CircuitDisconnected { address, reason } => {
                        info!("🔌 Circuit disconnected: {} ({})", address, reason);
                    }
                    NetworkEvent::Disconnected { reason } => {
                        info!("👋 Disconnected: {}", reason);
                    }
                    NetworkEvent::Error { error } => {
                        error!("❌ Network error: {}", error);
                    }
                }
            }
        });
        
        // Connect to the network
        let session = self.network_manager.connect(credentials).await?;
        info!("✅ Login successful! Session: {}", session.session_id);
        
        // Wait for connection to be ready
        while self.network_manager.status().await != NetworkStatus::Connected {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        
        info!("🌟 Client is ready!");
        Ok(())
    }
    
    /// Send a chat message
    pub async fn send_chat(&self, message: &str, channel: u32) -> NetworkResult<()> {
        let circuit = self.network_manager.primary_circuit().await
            .ok_or_else(|| crate::networking::NetworkError::Transport { 
                reason: "No primary circuit".to_string() 
            })?;
        
        let session = self.network_manager.session().await
            .ok_or_else(|| crate::networking::NetworkError::AuthenticationFailed { 
                reason: "No session".to_string() 
            })?;
        
        use crate::networking::packets::generated::{ChatFromViewer, ChatDataBlock, AgentDataBlock};
        
        let chat_packet = ChatFromViewer {
            agent_data: AgentDataBlock {
                agent_id: session.agent_id,
                session_id: session.session_id,
                circuit_code: session.circuit_code,
            },
            chat_data: ChatDataBlock {
                message: crate::networking::packets::types::LLVariable1::from_string(message),
                chat_type: 1, // Normal chat
                channel,
            },
        };
        
        circuit.send(&chat_packet).await?;
        info!("💬 Sent chat: {}", message);
        
        Ok(())
    }
    
    /// Run the client event loop
    pub async fn run(&self) -> NetworkResult<()> {
        info!("🔄 Starting client event loop...");
        
        // In a real implementation, this would:
        // 1. Process incoming packets through the handler registry
        // 2. Handle reconnection logic
        // 3. Manage multiple circuits
        // 4. Provide a proper event loop
        
        // For now, just keep the connection alive
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            
            let status = self.network_manager.status().await;
            if status == NetworkStatus::Disconnected {
                break;
            }
        }
        
        Ok(())
    }
    
    /// Disconnect from Second Life
    pub async fn disconnect(&mut self) -> NetworkResult<()> {
        info!("👋 Disconnecting...");
        self.network_manager.disconnect().await?;
        Ok(())
    }
}

/// Example usage
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn example_client_usage() {
        // Initialize tracing
        tracing_subscriber::fmt::init();
        
        // Create and initialize client
        let mut client = ImprovedClient::new();
        client.initialize().await.unwrap();
        
        // In a real test, we'd connect and interact
        // client.connect("Test User".to_string(), "password".to_string()).await.unwrap();
        // client.send_chat("Hello, Second Life!", 0).await.unwrap();
        // client.run().await.unwrap();
        
        info!("✅ Client example completed");
    }
}