use super::{HandlerContext, TypedPacketHandler};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::generated::RegionHandshake;
use crate::networking::client::HandshakeEvent;
use async_trait::async_trait;
use tracing::info;

pub struct RegionHandshakeHandler;

impl RegionHandshakeHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<RegionHandshake> for RegionHandshakeHandler {
    async fn handle_typed(&self, packet: RegionHandshake, context: &HandlerContext) -> NetworkResult<()> {
        info!("Received RegionHandshake for region: {}", String::from_utf8_lossy(&packet.sim_name));
        info!("Region flags: {:x}", packet.region_flags);
        info!("Water height: {}", packet.water_height);
        // Send handshake event
        context.handshake_tx.send(HandshakeEvent::RegionHandshake).await
            .map_err(|e| NetworkError::Other { reason: format!("Failed to send RegionHandshake event: {}", e) })?;
        Ok(())
    }
}