//! Handlers for login and connection-related packets

use super::{HandlerContext, TypedPacketHandler};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::client::HandshakeEvent;
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
        
        info!("Region info: {}", String::from_utf8_lossy(&packet.sim_name));
        info!("Region flags: {:x}", packet.region_flags);
        info!("Water height: {}", packet.water_height);
        
        // Send RegionHandshakeReply
        let reply = RegionHandshakeReply {
            agent_id: context.agent_id,
            session_id: context.session_id,
            flags: 0, // Client flags - 0 for basic functionality
        };
        
        context.circuit.send_reliable(&reply, std::time::Duration::from_secs(5)).await?;
        info!("Sent RegionHandshakeReply");
        
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
            agent_id: context.agent_id,
            session_id: context.session_id,
            circuit_code: context.circuit.circuit_code(),
            gen_counter: 0,
            throttles: crate::networking::packets::types::LLVariable1::new(throttle_data),
        };
        
        context.circuit.send_reliable(&agent_throttle, std::time::Duration::from_secs(5)).await?;
        debug!("Sent AgentThrottle");
        
        // 2. AgentHeightWidth
        let agent_height_width = AgentHeightWidth {
            agent_id: context.agent_id,
            session_id: context.session_id,
            circuit_code: context.circuit.circuit_code(),
            gen_counter: 0,
            height: 200, // Default height in cm
            width: 60,   // Default width in cm
        };
        
        context.circuit.send_reliable(&agent_height_width, std::time::Duration::from_secs(5)).await?;
        debug!("Sent AgentHeightWidth");
        
        // 3. AgentUpdate - the critical packet that "really gets things going"
        let agent_update = AgentUpdate {
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
        };
        
        // AgentUpdate is sent unreliably and frequently
        context.circuit.send(&agent_update).await?;
        debug!("Sent AgentUpdate");
        
        info!("Completed RegionHandshake sequence - login should now proceed");
        
        context.handshake_tx.send(HandshakeEvent::RegionHandshake).await
            .map_err(|e| NetworkError::Other { reason: format!("Failed to send RegionHandshake event: {}", e) })?;

        Ok(())
    }
}

/// Handler for StartPingCheck packets - critical for circuit health
pub struct StartPingCheckHandler;

impl StartPingCheckHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<StartPingCheck> for StartPingCheckHandler {
    async fn handle_typed(&self, packet: StartPingCheck, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received StartPingCheck from {} with PingID: {}", 
               context.circuit.address(), packet.ping_id);
        
        // Respond with CompletePingCheck
        let reply = CompletePingCheck {
            ping_id: packet.ping_id,
        };
        
        // Send the ping reply immediately (unreliable)
        context.circuit.send(&reply).await?;
        debug!("Sent CompletePingCheck reply with PingID: {}", packet.ping_id);
        
        Ok(())
    }
}

/// Handler for CompletePingCheck packets - ping response
pub struct CompletePingCheckHandler;

impl CompletePingCheckHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<CompletePingCheck> for CompletePingCheckHandler {
    async fn handle_typed(&self, packet: CompletePingCheck, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received CompletePingCheck from {} with PingID: {}", 
               context.circuit.address(), packet.ping_id);
        
        // Handle ping response
        context.circuit.handle_ping_response(packet.ping_id).await;
        
        Ok(())
    }
}

/// Handler for ObjectUpdate packets - world object synchronization  
pub struct ObjectUpdateHandler;

impl ObjectUpdateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<ObjectUpdate> for ObjectUpdateHandler {
    async fn handle_typed(&self, packet: ObjectUpdate, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received ObjectUpdate from {} with {} objects", 
               context.circuit.address(), packet.object_data.len());
        
        // Process object updates
        for (i, obj) in packet.object_data.into_iter().enumerate() {
                                                debug!("Object {}: Available fields: {:?}", i, obj);
        }
        
        // Object updates usually don't require immediate response
        // The viewer processes these to update the world state
        
        Ok(())
    }
}

/// Handler for LayerData packets - terrain information
pub struct LayerDataHandler;

impl LayerDataHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<LayerData> for LayerDataHandler {
    async fn handle_typed(&self, packet: LayerData, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received LayerData from {} type={} size={}", 
               context.circuit.address(), 
               packet.r#type,
               packet.data.data.len());
        
        // Process layer data (terrain, water, etc.)
        match packet.r#type {
            0 => debug!("Land layer data received"),
            1 => debug!("Water layer data received"), 
            2 => debug!("Wind layer data received"),
            3 => debug!("Cloud layer data received"),
            _ => debug!("Unknown layer type: {}", packet.r#type),
        }
        
        Ok(())
    }
}

/// Handler for CoarseLocationUpdate packets - avatar tracking
pub struct CoarseLocationUpdateHandler;

impl CoarseLocationUpdateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<CoarseLocationUpdate> for CoarseLocationUpdateHandler {
    async fn handle_typed(&self, packet: CoarseLocationUpdate, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received CoarseLocationUpdate from {} with {} locations", 
               context.circuit.address(), packet.location.len());
        
        // Process avatar locations
        for (i, location) in packet.location.iter().enumerate() {
            debug!("Avatar {}: location={:?}", i, location);
        }
        
        // Process you/prey data if present
        debug!("You: {:?}", packet.you);
        debug!("Prey: {:?}", packet.prey);
        debug!("Agent data: {:?}", packet.agent_data);
        
        Ok(())
    }
}

/// Handler for EconomyData packets - economic information
pub struct EconomyDataHandler;

impl EconomyDataHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<EconomyData> for EconomyDataHandler {
    async fn handle_typed(&self, packet: EconomyData, context: &HandlerContext) -> NetworkResult<()> {
        info!("Received EconomyData from {}", context.circuit.address());
        
        debug!("Economy info - Object count: {}, Price public: {}", 
               packet.object_count, 
               packet.price_public_object_decay);
        
        // Economy data doesn't typically require a response
        // It provides information about land costs, upload fees, etc.
        
        Ok(())
    }
}

/// Handler for UUIDNameReply packets - avatar name resolution
pub struct UUIDNameReplyHandler;

impl UUIDNameReplyHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<UUIDNameReply> for UUIDNameReplyHandler {
    async fn handle_typed(&self, packet: UUIDNameReply, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received UUIDNameReply from {} with {} names", 
               context.circuit.address(), packet.uuidname_block.len());
        
        // Process UUID to name mappings
        for name_block in &packet.uuidname_block {
            debug!("UUID name block: {:?}", name_block);
        }
        
        Ok(())
    }
}

/// Handler for EnableSimulator packets - multi-sim regions
pub struct EnableSimulatorHandler;

impl EnableSimulatorHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<EnableSimulator> for EnableSimulatorHandler {
    async fn handle_typed(&self, packet: EnableSimulator, context: &HandlerContext) -> NetworkResult<()> {
        info!("Received EnableSimulator from {}", context.circuit.address());
        
        debug!("Enable simulator: {} at {}:{}", 
               packet.handle,
               packet.ip.to_std_addr(), 
               packet.port.to_host_order());
        
        // EnableSimulator tells us about adjacent regions
        // In a full implementation, we'd establish circuits to these
        
        Ok(())
    }
}

/// Handler for AgentMovementComplete packets - CRITICAL for handshake completion
pub struct AgentMovementCompleteHandler;

impl AgentMovementCompleteHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<AgentMovementComplete> for AgentMovementCompleteHandler {
    async fn handle_typed(&self, packet: AgentMovementComplete, context: &HandlerContext) -> NetworkResult<()> {
        info!("Received AgentMovementComplete from {} - completing handshake!", context.circuit.address());
        
        debug!("Agent position: {:?}", packet.position);
        debug!("Channel version: {:?}", packet.channel_version);
        
        // Configuration packets (AgentFOV, AgentThrottle, AgentHeightWidth) are now sent
        // earlier in the handshake sequence in client.rs to match homunculus timing.
        // This handler focuses on the post-movement completion sequence.
        
        // Send initial AgentUpdate packets (following homunculus pattern)
        // This sequence handles the "squatting animation" issue homunculus mentions
        let control_flags_sequence = [0, 0x40000000, 0]; // NONE, FINISH_ANIM, NONE
        
        for control_flags in control_flags_sequence {
            let agent_update = AgentUpdate {
                agent_id: context.agent_id,
                session_id: context.session_id,
                body_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                head_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                state: 0,
                camera_center: crate::networking::packets::types::LLVector3::new(128.0, 128.0, 25.0),
                camera_at_axis: crate::networking::packets::types::LLVector3::new(1.0, 0.0, 0.0),
                camera_left_axis: crate::networking::packets::types::LLVector3::new(0.0, 1.0, 0.0),
                camera_up_axis: crate::networking::packets::types::LLVector3::new(0.0, 0.0, 1.0),
                far: 256.0,
                control_flags,
                flags: 0,
            };
            
            // AgentUpdate is sent unreliably
            context.circuit.send(&agent_update).await?;
            debug!("Sent AgentUpdate with control_flags: 0x{:08x}", control_flags);
            
            // Add small delay between updates (homunculus uses 1000ms)
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        
        info!("🎉 Handshake sequence completed successfully! Connection should now be stable.");
        
        // Set circuit to Ready state
        context.circuit.set_state(crate::networking::circuit::CircuitState::Ready).await?;
        
        context.handshake_tx.send(HandshakeEvent::AgentMovementComplete).await
            .map_err(|e| NetworkError::Other { reason: format!("Failed to send AgentMovementComplete event: {}", e) })?;

        Ok(())
    }
}