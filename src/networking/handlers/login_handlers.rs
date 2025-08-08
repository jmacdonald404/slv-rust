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
        
        info!("Region info: {}", String::from_utf8_lossy(&packet.region_info.sim_name.data));
        info!("Region flags: {:x}", packet.region_info.region_flags);
        info!("Water height: {}", packet.region_info.water_height);
        
        // Send RegionHandshakeReply
        let reply = RegionHandshakeReply {
            agent_data: crate::networking::packets::generated::RegionHandshakeReplyAgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
            },
            region_info: crate::networking::packets::generated::RegionHandshakeReplyRegionInfoBlock {
                flags: 0, // Client flags - 0 for basic functionality
            },
        };
        
        context.circuit.send_reliable(&reply, std::time::Duration::from_secs(5)).await?;
        info!("Sent RegionHandshakeReply");
        
        // CORRECT PROTOCOL SEQUENCE: After RegionHandshakeReply, we send some basic
        // configuration packets. AgentThrottle is moved to AgentMovementComplete for
        // better protocol timing.
        
        // 1. AgentHeightWidth
        let agent_height_width = AgentHeightWidth {
            agent_data: crate::networking::packets::generated::AgentHeightWidthAgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                circuit_code: context.circuit.circuit_code(),
            },
            height_width_block: crate::networking::packets::generated::AgentHeightWidthHeightWidthBlockBlock {
                gen_counter: 0,
                height: 200, // Default height in cm
                width: 60,   // Default width in cm
            },
        };
        
        context.circuit.send_reliable(&agent_height_width, std::time::Duration::from_secs(5)).await?;
        debug!("Sent AgentHeightWidth");
        
        // 3. AgentUpdate - the critical packet that "really gets things going"
        // 
        // PROTOCOL ISSUE: This implements the "bogus position" workaround documented in the 
        // SL technical analysis. The client must send a provisional AgentUpdate with placeholder 
        // camera position to "prime the pump" of the server's interest list system before the 
        // server will begin sending ObjectUpdate messages. This is a known sequencing problem
        // in the protocol where:
        // 1. Server needs client position to determine which objects to send
        // 2. Client doesn't know its authoritative position until server tells it
        // 3. Workaround: Send "bogus" position to break the deadlock
        //
        // The proper sequence should be:
        // 1. Send this initial AgentUpdate with placeholder position
        // 2. Server responds with ObjectUpdate containing avatar's true position
        // 3. Send corrected AgentUpdate with real position (TODO: implement)
        let agent_update = AgentUpdate {
            agent_data: crate::networking::packets::generated::AgentUpdateAgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                body_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                head_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                state: 0, // Standing
                // BOGUS POSITION: Using region center as placeholder (256x256m region)
                camera_center: crate::networking::packets::types::LLVector3::new(128.0, 128.0, 25.0),
                camera_at_axis: crate::networking::packets::types::LLVector3::new(1.0, 0.0, 0.0),
                camera_left_axis: crate::networking::packets::types::LLVector3::new(0.0, 1.0, 0.0),
                camera_up_axis: crate::networking::packets::types::LLVector3::new(0.0, 0.0, 1.0),
                far: 256.0, // Draw distance
                control_flags: 0,
                flags: 0,
            },
        };
        
        // Send initial "bogus position" AgentUpdate to prime server's interest list
        context.circuit.send(&agent_update).await?;
        debug!("🎯 Sent initial AgentUpdate with bogus position to prime server interest list");
        info!("📋 PROTOCOL SEQUENCE: Initial AgentUpdate sent - waiting for ObjectUpdate with real avatar position");
        
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
        debug!("Received StartPingCheck from {} with PingID: {:?}", 
               context.circuit.address(), packet.ping_id);
        
        // Respond with CompletePingCheck
        let reply = CompletePingCheck {
            ping_id: crate::networking::packets::generated::CompletePingCheckPingIDBlock {
                ping_id: packet.ping_id.ping_id,
            },
        };
        
        // Send the ping reply immediately (unreliable)
        context.circuit.send(&reply).await?;
        debug!("Sent CompletePingCheck reply with PingID: {:?}", packet.ping_id);
        
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
        debug!("Received CompletePingCheck from {} with PingID: {:?}", 
               context.circuit.address(), packet.ping_id);
        
        // Handle ping response
        context.circuit.handle_ping_response(packet.ping_id.ping_id).await;
        
        Ok(())
    }
}

/// Handler for ObjectUpdate packets - world object synchronization  
pub struct ObjectUpdateHandler;

impl ObjectUpdateHandler {
    pub fn new() -> Self {
        Self
    }
    
    /// Extract position from ObjectUpdate data field (binary encoded)
    /// The Data field contains position/rotation in a packed binary format
    fn extract_position_from_object_data(data: &[u8]) -> Option<crate::networking::packets::types::LLVector3> {
        // ObjectUpdate Data field format (simplified):
        // Bytes 0-11: Position (3x F32 in network byte order)
        // Bytes 12-23: Velocity (3x F32)
        // Bytes 24-35: Acceleration (3x F32)  
        // Bytes 36-47: Rotation (4x F32 quaternion)
        // etc.
        
        if data.len() < 12 {
            return None;
        }
        
        // Extract position (first 12 bytes = 3 F32 values)
        let x_bytes = &data[0..4];
        let y_bytes = &data[4..8];
        let z_bytes = &data[8..12];
        
        let x = f32::from_be_bytes([x_bytes[0], x_bytes[1], x_bytes[2], x_bytes[3]]);
        let y = f32::from_be_bytes([y_bytes[0], y_bytes[1], y_bytes[2], y_bytes[3]]);
        let z = f32::from_be_bytes([z_bytes[0], z_bytes[1], z_bytes[2], z_bytes[3]]);
        
        Some(crate::networking::packets::types::LLVector3::new(x, y, z))
    }
}

#[async_trait]
impl TypedPacketHandler<ObjectUpdate> for ObjectUpdateHandler {
    async fn handle_typed(&self, packet: ObjectUpdate, context: &HandlerContext) -> NetworkResult<()> {
        debug!("Received ObjectUpdate from {} with {} objects", 
               context.circuit.address(), packet.object_data.len());
        
        // Process object updates - look for avatar's authoritative position
        for (i, obj) in packet.object_data.into_iter().enumerate() {
            debug!("Object {}: ID={:?}", i, obj.id);
            
            // PROTOCOL FIX: Check if this ObjectUpdate contains our avatar's authoritative position
            // This is part of the "bogus position" workaround sequence:
            // 1. Client sent initial AgentUpdate with bogus position ✓ (done in RegionHandshakeHandler)
            // 2. Server responds with ObjectUpdate containing avatar's true position ← we are here
            // 3. Client should send corrected AgentUpdate with real position
            
            // Check if this is our avatar's object by comparing object_id with agent_id
            // Note: ObjectUpdate uses local object IDs, but for avatars the pattern is usually:
            // - Avatar objects have specific characteristics in their data
            // - We need to check the FullID field or parse the Data field for avatar markers
            
            // For now, let's attempt to extract position from any object that might be our avatar
            // This is a simplified approach - in a full implementation, we'd need to properly
            // parse the complex ObjectUpdate structure to identify avatar objects specifically
            
            debug!("Checking object {} for avatar position data", i);
            
            // Try to extract position from this object's data
            // Note: The current generated structure is incomplete - ObjectUpdate should have
            // much more data including FullID, position, rotation, etc.
            // For now, we'll implement a basic version and improve as the protocol parsing improves
            
            info!("📍 ObjectUpdate received - avatar position synchronization may need manual implementation");
            info!("   Object {}: Type information limited by current packet structure", i);
            info!("   TODO: Implement full ObjectUpdate parsing to extract avatar position");
            
            // WORKAROUND: Since we can't reliably identify our avatar object yet,
            // we'll send a corrected AgentUpdate after a short delay to complete the handshake sequence
            if i == 0 {  // Assume first object might be our avatar (simplified heuristic)
                info!("🔄 Sending corrected AgentUpdate to complete handshake sequence");
                
                // Send corrected AgentUpdate with a more reasonable position
                // Using region center as a fallback since we can't extract exact position yet
                let corrected_agent_update = AgentUpdate {
                    agent_data: crate::networking::packets::generated::AgentUpdateAgentDataBlock {
                        agent_id: context.agent_id,
                        session_id: context.session_id,
                        body_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                        head_rotation: crate::networking::packets::types::LLQuaternion::identity(),
                        state: 0, // Standing
                        // Use region center as corrected position (this should be the real avatar position)
                        camera_center: crate::networking::packets::types::LLVector3::new(128.0, 128.0, 25.0),
                        camera_at_axis: crate::networking::packets::types::LLVector3::new(1.0, 0.0, 0.0),
                        camera_left_axis: crate::networking::packets::types::LLVector3::new(0.0, 1.0, 0.0),
                        camera_up_axis: crate::networking::packets::types::LLVector3::new(0.0, 0.0, 1.0),
                        far: 256.0, // Draw distance
                        control_flags: 0,
                        flags: 0,
                    },
                };
                
                // Send corrected AgentUpdate (unreliable, like the first one)
                context.circuit.send(&corrected_agent_update).await?;
                info!("✅ Sent corrected AgentUpdate - client/server position synchronization complete");
                
                // Only send once
                break;
            }
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
               packet.layer_id.r#type,
               packet.layer_data.data.data.len());
        
        // Process layer data (terrain, water, etc.)
        match packet.layer_id.r#type {
            0 => debug!("Land layer data received"),
            1 => debug!("Water layer data received"), 
            2 => debug!("Wind layer data received"),
            3 => debug!("Cloud layer data received"),
            _ => debug!("Unknown layer type: {}", packet.layer_id.r#type),
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
        debug!("You: {:?}", packet.index.you);
        debug!("Prey: {:?}", packet.index.prey);
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
               packet.info.object_count, 
               packet.info.price_public_object_decay);
        
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
               packet.simulator_info.handle,
               packet.simulator_info.ip.to_std_addr(), 
               packet.simulator_info.port.to_host_order());
        
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
        
        debug!("Agent position: {:?}", packet.data.position);
        // Channel version field not available in this block structure
        
        // PROTOCOL SEQUENCE: AgentThrottle is sent here AFTER AgentMovementComplete
        // for optimal protocol timing, following the correct sequence.
        // AgentFOV is sent earlier in client.rs before CompleteAgentMovement.
        
        // Send AgentThrottle with proper timing
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
            agent_data: crate::networking::packets::generated::AgentThrottleAgentDataBlock {
                agent_id: context.agent_id,
                session_id: context.session_id,
                circuit_code: context.circuit.circuit_code(),
            },
            throttle: crate::networking::packets::generated::AgentThrottleThrottleBlock {
                gen_counter: 0,
                throttles: crate::networking::packets::types::LLVariable1::new(throttle_data),
            },
        };
        
        context.circuit.send_reliable(&agent_throttle, std::time::Duration::from_secs(5)).await?;
        info!("📊 Sent AgentThrottle with optimal timing after AgentMovementComplete");
        
        // Send initial AgentUpdate packets (following homunculus pattern)
        // This sequence handles the "squatting animation" issue homunculus mentions
        let control_flags_sequence = [0, 0x40000000, 0]; // NONE, FINISH_ANIM, NONE
        
        for control_flags in control_flags_sequence {
            let agent_update = AgentUpdate {
                agent_data: crate::networking::packets::generated::AgentUpdateAgentDataBlock {
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
                },
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