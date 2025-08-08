//! Handlers for agent-related packets

use super::{HandlerContext, TypedPacketHandler};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::generated::*;
use async_trait::async_trait;
use tracing::{info, debug};

/// Handler for PacketAck packets
pub struct PacketAckHandler;

impl PacketAckHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<PacketAck> for PacketAckHandler {
    async fn handle_typed(&self, packet: PacketAck, context: &HandlerContext) -> NetworkResult<()> {
        info!("Received PacketAck with {} acknowledgments", packet.packets.len());
        
        // Forward acknowledgments to the circuit's acknowledger
        context.circuit.handle_ack(&packet).await;
        
        Ok(())
    }
}

/// Handler for ViewerEffect packets (incoming effects from other agents)
pub struct ViewerEffectHandler;

impl ViewerEffectHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<ViewerEffect> for ViewerEffectHandler {
    async fn handle_typed(&self, packet: ViewerEffect, _context: &HandlerContext) -> NetworkResult<()> {
        for effect in &packet.effect {
            info!("🎭 Received ViewerEffect from agent {}: Type={}, Duration={}s", 
                effect.agent_id, effect.r#type, effect.duration);
            
            // Log more details for debugging
            debug!("   Effect ID: {}", effect.id);
            debug!("   Color: {:?}", effect.color);
            debug!("   TypeData: {} bytes", effect.type_data.data.len());
            
            // Here you could:
            // - Decode the TypeData based on effect_type
            // - Trigger visual effects in the renderer
            // - Store effect state for animation
            // - Forward to UI components for display
        }
        
        Ok(())
    }
}