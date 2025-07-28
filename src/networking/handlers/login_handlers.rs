//! Handlers for login and connection-related packets

use super::{HandlerContext, TypedPacketHandler};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::generated::*;
use async_trait::async_trait;
use tracing::{debug, info};

/// Handler for RegionHandshake packets
pub struct RegionHandshakeHandler;

impl RegionHandshakeHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<RegionHandshake> for RegionHandshakeHandler {
    async fn handle_typed(&self, packet: RegionHandshake, context: &HandlerContext) -> NetworkResult<()> {
        info!("Received RegionHandshake from {}", context.circuit.address());
        
        debug!("Region info: {}", packet.region_info.sim_name.to_string().unwrap_or_default());
        debug!("Region flags: {:x}", packet.region_info.region_flags);
        debug!("Water height: {}", packet.region_info.water_height);
        
        // Send RegionHandshakeReply
        let reply = RegionHandshakeReply {
            agent_data: AgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                circuit_code: context.circuit.circuit_code(),
            },
            region_info: RegionHandshakeReplyRegionInfo {
                flags: 0, // Client flags - 0 for basic functionality
            },
        };
        
        context.circuit.send_reliable(&reply, std::time::Duration::from_secs(5)).await?;
        debug!("Sent RegionHandshakeReply");
        
        // After RegionHandshakeReply, we need to send the critical packets
        // mentioned in the protocol implementation analysis:
        
        // 1. AgentThrottle
        let throttle_data = {
            // Create throttle data: 7 floats (28 bytes) for different traffic types
            // [resend, land, wind, cloud, task, texture, asset]
            let throttles = [
                150000.0f32, // resend
                170000.0f32, // land  
                0.0f32,      // wind (usually 0)
                0.0f32,      // cloud (usually 0)
                280000.0f32, // task (objects)
                446000.0f32, // texture
                220000.0f32, // asset
            ];
            
            let mut data = Vec::with_capacity(28);
            for throttle in &throttles {
                data.extend_from_slice(&throttle.to_le_bytes());
            }
            data
        };
        
        let agent_throttle = AgentThrottle {
            agent_data: AgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                circuit_code: context.circuit.circuit_code(),
            },
            throttle: ThrottleBlock {
                gen_counter: 0,
                throttles: crate::networking::packets::types::LLVariable1::new(throttle_data),
            },
        };
        
        context.circuit.send_reliable(&agent_throttle, std::time::Duration::from_secs(5)).await?;
        debug!("Sent AgentThrottle");
        
        // 2. AgentHeightWidth
        let agent_height_width = AgentHeightWidth {
            agent_data: AgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                circuit_code: context.circuit.circuit_code(),
            },
            height_width_block: HeightWidthBlock {
                gen_counter: 0,
                height: 200, // Default height in cm
                width: 60,   // Default width in cm
            },
        };
        
        context.circuit.send_reliable(&agent_height_width, std::time::Duration::from_secs(5)).await?;
        debug!("Sent AgentHeightWidth");
        
        // 3. AgentUpdate - the critical packet that "really gets things going"
        let agent_update = AgentUpdate {
            agent_data: AgentUpdateDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                body_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                head_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                state: 0, // Standing
                camera_center: crate::networking::packets::types::LLVector3::new(128.0, 128.0, 25.0),
                camera_at_axis: crate::networking::packets::types::LLVector3::new(1.0, 0.0, 0.0),
                camera_left_axis: crate::networking::packets::types::LLVector3::new(0.0, 1.0, 0.0),
                camera_up_axis: crate::networking::packets::types::LLVector3::new(0.0, 0.0, 1.0),
                far: 256.0, // Draw distance
                control_flags: 0,
                flags: 0,
            },
        };
        
        // AgentUpdate is sent unreliably and frequently
        context.circuit.send(&agent_update).await?;
        debug!("Sent AgentUpdate");
        
        info!("Completed RegionHandshake sequence - login should now proceed");
        
        Ok(())
    }
}