//! Handlers for agent-related packets

use super::{HandlerContext, TypedPacketHandler};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::generated::*;
use async_trait::async_trait;
use tracing::info;

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