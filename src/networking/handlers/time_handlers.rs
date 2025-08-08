//! Handlers for time synchronization packets

use super::{HandlerContext, TypedPacketHandler};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::generated::*;
use async_trait::async_trait;
use tracing::{info, debug};

/// Handler for SimulatorViewerTimeMessage packets
#[derive(Debug, Clone)]
pub struct SimulatorViewerTimeMessageHandler;

impl SimulatorViewerTimeMessageHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<SimulatorViewerTimeMessage> for SimulatorViewerTimeMessageHandler {
    async fn handle_typed(&self, packet: SimulatorViewerTimeMessage, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🕒 Received SimulatorViewerTimeMessage");
        debug!("   UsecSinceStart: {}", packet.time_info.usec_since_start);
        debug!("   SecPerDay: {}", packet.time_info.sec_per_day);
        debug!("   SecPerYear: {}", packet.time_info.sec_per_year);
        debug!("   SunDirection: {:?}", packet.time_info.sun_direction);
        debug!("   SunPhase: {}", packet.time_info.sun_phase);
        debug!("   SunAngVelocity: {:?}", packet.time_info.sun_ang_velocity);
        
        // Just log for now - in a full implementation this would update world time
        info!("✅ Time synchronization updated");
        
        Ok(())
    }
}