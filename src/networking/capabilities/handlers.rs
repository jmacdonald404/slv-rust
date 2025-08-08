//! Packet handlers for capability-related messages
//! 
//! These handlers process packets that provide capability information
//! or trigger capability-based operations.

use super::{CapabilitiesManager, SessionInfo};
use crate::networking::{NetworkResult};
use crate::networking::handlers::{TypedPacketHandler, HandlerContext};
use crate::networking::packets::{generated::*};
use tracing::{info, debug, warn, error};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handler for RegionHandshakeReply packets that contain seed capability
pub struct RegionHandshakeReplyHandler {
    capabilities_manager: Arc<RwLock<Option<CapabilitiesManager>>>,
}

impl RegionHandshakeReplyHandler {
    pub fn new() -> Self {
        Self {
            capabilities_manager: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Set the capabilities manager for this handler
    pub async fn set_capabilities_manager(&self, manager: CapabilitiesManager) {
        let mut caps = self.capabilities_manager.write().await;
        *caps = Some(manager);
    }
}

#[async_trait]
impl TypedPacketHandler<RegionHandshakeReply> for RegionHandshakeReplyHandler {
    async fn handle_typed(&self, handshake_reply: RegionHandshakeReply, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🤝 Received RegionHandshakeReply with capabilities");
        
        debug!("🌍 Region handshake reply: Agent={}, Session={}", 
               handshake_reply.agent_data.agent_id,
               handshake_reply.agent_data.session_id);
        
        // Extract seed capability from the handshake reply
        // The seed capability is typically in the region_info.sim_access field as a URL
        // TODO: Parse the actual seed capability URL from the packet structure
        // For now, we'll log that we received the handshake
        
        debug!("✅ Region handshake completed - capabilities system ready");
        
        // TODO: Extract and process seed capability
        // let seed_cap_url = parse_seed_capability(&handshake_reply);
        // if let Some(manager) = &*self.capabilities_manager.read().await {
        //     manager.initialize_from_seed(seed_cap_url).await?;
        // }
        
        Ok(())
    }
}

/// Handler for EstateOwnerMessage packets that may contain capability updates
pub struct EstateOwnerMessageHandler;

impl EstateOwnerMessageHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<EstateOwnerMessage> for EstateOwnerMessageHandler {
    async fn handle_typed(&self, estate_message: EstateOwnerMessage, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🏰 Received EstateOwnerMessage");
        
        debug!("🏰 Method: {:?}", estate_message.method_data.method);
        debug!("   Invoice: {}", estate_message.method_data.invoice);
        
        // Process parameter data
        for param in &estate_message.param_list {
            debug!("   Param: {:?}", param.parameter);
        }
        
        // TODO: Check if this message contains capability updates
        // Some estate messages may provide new capability URLs
        
        Ok(())
    }
}

/// Capability integration helper functions
impl CapabilitiesManager {
    /// Initialize capabilities from a seed capability URL
    pub async fn initialize_from_seed(&self, seed_url: &str) -> Result<(), super::CapabilityError> {
        info!("🌱 Initializing capabilities from seed: {}", seed_url);
        
        // Make request to seed capability to get full capability list
        let client = self.http_client.clone();
        
        let response = client.get(seed_url)
            .header("User-Agent", "slv-rust/0.3.0")
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| super::CapabilityError::HttpError(e.to_string()))?;
            
        let status = response.status().as_u16();
        let response_text = response.text().await
            .map_err(|e| super::CapabilityError::HttpError(e.to_string()))?;
        
        if status < 200 || status >= 300 {
            return Err(super::CapabilityError::HttpError(format!("HTTP {}", status)));
        }
        
        let capabilities: HashMap<String, String> = serde_json::from_str(&response_text)
            .map_err(|e| super::CapabilityError::ParseError(e.to_string()))?;
        
        self.register_capabilities(capabilities).await?;
        
        info!("🌱 Successfully initialized capabilities from seed");
        Ok(())
    }
    
    /// Start the event queue polling loop
    pub async fn start_event_queue_polling(&self) -> Result<(), super::CapabilityError> {
        if self.get_capability(super::well_known_capabilities::EVENT_QUEUE_GET).await.is_none() {
            warn!("📨 EventQueueGet capability not available, cannot start polling");
            return Ok(());
        }
        
        info!("📨 Starting event queue polling");
        
        let manager = Arc::new(self.clone());
        tokio::spawn(async move {
            let mut ack_id: Option<i64> = None;
            
            loop {
                match manager.poll_event_queue(ack_id).await {
                    Ok(event_response) => {
                        // Update ack ID for next poll
                        ack_id = Some(event_response.id);
                        
                        // Process events
                        for event in &event_response.events {
                            debug!("📨 Event: {} - {:?}", event.message, event.body);
                            
                            // TODO: Dispatch events to appropriate handlers
                            // This would integrate with the event bus architecture per netplan.md
                        }
                        
                        // Check if we should continue polling
                        if event_response.done {
                            debug!("📨 Event queue polling completed");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("📨 Event queue polling error: {}", e);
                        // Wait before retrying
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
                
                // Wait before next poll - EventQueue should be polled every 5-10 seconds, not aggressively
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        
        Ok(())
    }
}

// Helper to clone the capabilities manager (needed for async tasks)
impl Clone for CapabilitiesManager {
    fn clone(&self) -> Self {
        Self {
            capabilities: Arc::clone(&self.capabilities),
            http_client: self.http_client.clone(),
            session_info: self.session_info.clone(),
        }
    }
}