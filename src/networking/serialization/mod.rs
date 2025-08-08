//! Second Life packet serialization and deserialization
//! 
//! This module provides exact binary compatibility with the Second Life protocol,
//! handling packet headers, flags, zerocoding, and reliable packet acknowledgment.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::{Packet, PacketFrequency, PacketWrapper};
use crate::networking::packets::generated::{
    UseCircuitCode, CompleteAgentMovement, RegionHandshakeReply, AgentThrottle, 
    AgentHeightWidth, AgentAnimation, SetAlwaysRun, MuteListRequest, 
    MoneyBalanceRequest, UUIDNameRequest, AgentFOV, ViewerEffect, AgentUpdate, PacketAck
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::any::Any;
use tracing::{info, warn, debug};

pub mod packet_buffer;
pub mod zerocode;
pub mod template_deserializer;
pub mod template_registry;

pub use packet_buffer::PacketBuffer;

/// Maximum packet sequence number before wrapping
const MAX_SEQUENCE: u32 = 0x01000000;

/// Packet flags
const ACK_FLAG: u8 = 0x40;      // Reliable packet flag
const RESENT_FLAG: u8 = 0x20;    // Resent packet flag  
const ZEROCODED_FLAG: u8 = 0x80; // Zerocoded packet flag

/// Packet serializer that produces exact Second Life protocol format
pub struct PacketSerializer {
    sequence: u32,
}

impl PacketSerializer {
    pub fn new() -> Self {
        Self { sequence: 1 }
    }
    
    /// Get the current sequence number
    pub fn current_sequence(&self) -> u32 {
        self.sequence
    }
    
    /// Serialize a packet into the exact Second Life UDP format
    pub fn serialize<P: Packet>(&mut self, packet: &P, reliable: bool) -> NetworkResult<(Bytes, u32)> {
        let mut buffer = BytesMut::new();
        
        // Serialize packet data using proper Second Life format instead of bincode
        let packet_data = self.serialize_sl_packet(packet)?;
            
        // Apply zerocoding if enabled
        let final_data = if P::ZEROCODED {
            zerocode::encode(&packet_data)
        } else {
            packet_data
        };
        
        // Build header
        let sequence = self.next_sequence();
        self.write_header(&mut buffer, P::ID.try_into().unwrap(), P::FREQUENCY, reliable, P::ZEROCODED, sequence);
        
        // Append packet data
        buffer.extend_from_slice(&final_data);
        
        Ok((buffer.freeze(), sequence))
    }
    
    /// Serialize a packet using proper Second Life binary format
    fn serialize_sl_packet<P: Packet>(&self, packet: &P) -> NetworkResult<Vec<u8>> {
        use crate::networking::packets::generated::*;
        use byteorder::{WriteBytesExt, LittleEndian};
        
        let mut data = Vec::new();
        
        // Dispatch to specific packet serializers based on packet type
        // This uses runtime dispatch but ensures correct serialization
        let packet_any = packet as &dyn std::any::Any;
        
        if let Some(use_circuit_code) = packet_any.downcast_ref::<UseCircuitCode>() {
            self.serialize_use_circuit_code(use_circuit_code, &mut data)?;
        } else if let Some(complete_agent_movement) = packet_any.downcast_ref::<CompleteAgentMovement>() {
            self.serialize_complete_agent_movement(complete_agent_movement, &mut data)?;
        } else if let Some(region_handshake_reply) = packet_any.downcast_ref::<RegionHandshakeReply>() {
            self.serialize_region_handshake_reply(region_handshake_reply, &mut data)?;
        } else if let Some(agent_throttle) = packet_any.downcast_ref::<AgentThrottle>() {
            self.serialize_agent_throttle(agent_throttle, &mut data)?;
        } else if let Some(agent_height_width) = packet_any.downcast_ref::<AgentHeightWidth>() {
            self.serialize_agent_height_width(agent_height_width, &mut data)?;
        } else if let Some(agent_animation) = packet_any.downcast_ref::<AgentAnimation>() {
            self.serialize_agent_animation(agent_animation, &mut data)?;
        } else if let Some(set_always_run) = packet_any.downcast_ref::<SetAlwaysRun>() {
            self.serialize_set_always_run(set_always_run, &mut data)?;
        } else if let Some(mute_list_request) = packet_any.downcast_ref::<MuteListRequest>() {
            self.serialize_mute_list_request(mute_list_request, &mut data)?;
        } else if let Some(money_balance_request) = packet_any.downcast_ref::<MoneyBalanceRequest>() {
            self.serialize_money_balance_request(money_balance_request, &mut data)?;
        } else if let Some(uuid_name_request) = packet_any.downcast_ref::<UUIDNameRequest>() {
            self.serialize_uuid_name_request(uuid_name_request, &mut data)?;
        } else if let Some(agent_fov) = packet_any.downcast_ref::<AgentFOV>() {
            self.serialize_agent_fov(agent_fov, &mut data)?;
        } else if let Some(viewer_effect) = packet_any.downcast_ref::<ViewerEffect>() {
            self.serialize_viewer_effect(viewer_effect, &mut data)?;
        } else if let Some(agent_update) = packet_any.downcast_ref::<AgentUpdate>() {
            self.serialize_agent_update(agent_update, &mut data)?;
        } else if let Some(packet_ack) = packet_any.downcast_ref::<PacketAck>() {
            self.serialize_packet_ack(packet_ack, &mut data)?;
        } else {
            // Fallback to bincode for unimplemented packets
            let bincode_data = bincode::serialize(packet)
                .map_err(|e| NetworkError::PacketEncode { 
                    reason: format!("Failed to serialize packet with fallback bincode: {}", e) 
                })?;
            data.extend_from_slice(&bincode_data);
        }
        
        Ok(data)
    }
    
    /// Serialize UseCircuitCode packet in proper SL format
    fn serialize_use_circuit_code(&self, packet: &UseCircuitCode, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // UseCircuitCode format:
        // - Code: U32 (4 bytes, little-endian)
        // - SessionID: LLUUID (16 bytes)
        // - ID: LLUUID (16 bytes) 
        
        // Circuit code (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.circuit_code.code)
            .map_err(|e| NetworkError::PacketEncode { 
                reason: format!("Failed to write circuit code: {}", e) 
            })?;
        
        // Session ID (16 bytes)
        data.extend_from_slice(packet.circuit_code.session_id.as_bytes());
        
        // Agent ID (16 bytes)  
        data.extend_from_slice(packet.circuit_code.id.as_bytes());
        
        Ok(())
    }
    
    /// Serialize CompleteAgentMovement packet in proper SL format
    fn serialize_complete_agent_movement(&self, packet: &CompleteAgentMovement, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // CompleteAgentMovement format:
        // - AgentID: LLUUID (16 bytes)
        // - SessionID: LLUUID (16 bytes)
        // - CircuitCode: U32 (4 bytes, little-endian)
        
        // Agent ID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        
        // Session ID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        
        // Circuit code (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.agent_data.circuit_code)
            .map_err(|e| NetworkError::PacketEncode { 
                reason: format!("Failed to write circuit code: {}", e) 
            })?;
        
        Ok(())
    }

    /// Serialize RegionHandshakeReply packet in proper SL format
    fn serialize_region_handshake_reply(&self, packet: &RegionHandshakeReply, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes) 
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // Flags: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.region_info.flags)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write flags: {}", e) })?;
        Ok(())
    }

    /// Serialize AgentThrottle packet in proper SL format
    fn serialize_agent_throttle(&self, packet: &AgentThrottle, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // CircuitCode: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.agent_data.circuit_code)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write circuit code: {}", e) })?;
        // GenCounter: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.throttle.gen_counter)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write gen counter: {}", e) })?;
        // Throttles: Variable1 (1 byte length + data)
        data.write_u8(packet.throttle.throttles.data.len() as u8)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write throttle length: {}", e) })?;
        data.extend_from_slice(&packet.throttle.throttles.data);
        Ok(())
    }

    /// Serialize AgentHeightWidth packet in proper SL format  
    fn serialize_agent_height_width(&self, packet: &AgentHeightWidth, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // CircuitCode: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.agent_data.circuit_code)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write circuit code: {}", e) })?;
        // GenCounter: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.height_width_block.gen_counter)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write gen counter: {}", e) })?;
        // Height: U16 (2 bytes, little-endian)
        data.write_u16::<LittleEndian>(packet.height_width_block.height)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write height: {}", e) })?;
        // Width: U16 (2 bytes, little-endian)
        data.write_u16::<LittleEndian>(packet.height_width_block.width)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write width: {}", e) })?;
        Ok(())
    }

    /// Serialize AgentAnimation packet in proper SL format
    fn serialize_agent_animation(&self, packet: &AgentAnimation, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        
        // AnimationList variable block
        data.write_u8(packet.animation_list.len() as u8)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write animation list length: {}", e) })?;
        for anim in &packet.animation_list {
            // AnimID: LLUUID (16 bytes)
            data.extend_from_slice(anim.anim_id.as_bytes());
            // StartAnim: Bool (1 byte)
            data.write_u8(if anim.start_anim { 1 } else { 0 })
                .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write start_anim: {}", e) })?;
        }
        
        // PhysicalAvatarEventList variable block  
        data.write_u8(packet.physical_avatar_event_list.len() as u8)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write physical avatar event list length: {}", e) })?;
        for event in &packet.physical_avatar_event_list {
            // TypeData: Variable1 (length + data)
            data.write_u8(event.type_data.data.len() as u8)
                .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write type data length: {}", e) })?;
            data.extend_from_slice(&event.type_data.data);
        }
        Ok(())
    }

    /// Serialize SetAlwaysRun packet in proper SL format
    fn serialize_set_always_run(&self, packet: &SetAlwaysRun, data: &mut Vec<u8>) -> NetworkResult<()> {
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // AlwaysRun: Bool (1 byte)
        data.push(if packet.agent_data.always_run { 1 } else { 0 });
        Ok(())
    }

    /// Serialize MuteListRequest packet in proper SL format
    fn serialize_mute_list_request(&self, packet: &MuteListRequest, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // MuteCRC: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.mute_data.mute_crc)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write mute crc: {}", e) })?;
        Ok(())
    }

    /// Serialize MoneyBalanceRequest packet in proper SL format
    fn serialize_money_balance_request(&self, packet: &MoneyBalanceRequest, data: &mut Vec<u8>) -> NetworkResult<()> {
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // TransactionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.money_data.transaction_id.as_bytes());
        Ok(())
    }

    /// Serialize UUIDNameRequest packet in proper SL format
    fn serialize_uuid_name_request(&self, packet: &UUIDNameRequest, data: &mut Vec<u8>) -> NetworkResult<()> {
        // UUIDNameBlock variable block
        data.push(packet.uuidname_block.len() as u8);
        for block in &packet.uuidname_block {
            // ID: LLUUID (16 bytes)
            data.extend_from_slice(block.id.as_bytes());
        }
        Ok(())
    }

    /// Serialize AgentFOV packet in proper SL format
    fn serialize_agent_fov(&self, packet: &AgentFOV, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        // SessionID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        // CircuitCode: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.agent_data.circuit_code)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write circuit code: {}", e) })?;
        // GenCounter: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.fovblock.gen_counter)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write gen counter: {}", e) })?;
        // VerticalAngle: F32 (4 bytes, little-endian)
        data.write_f32::<LittleEndian>(packet.fovblock.vertical_angle)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write vertical angle: {}", e) })?;
        Ok(())
    }

    /// Serialize ViewerEffect packet in proper SL format
    /// Based on message_template.msg:
    /// - AgentData Single block: AgentID + SessionID
    /// - Effect Variable block: count + (ID + AgentID + Type + Duration + Color[4] + TypeData[Variable1]) for each effect
    fn serialize_viewer_effect(&self, packet: &ViewerEffect, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentData single block (32 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());   // LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.session_id.as_bytes()); // LLUUID (16 bytes)
        
        // Effect Variable block
        if packet.effect.len() > 255 {
            return Err(NetworkError::PacketEncode {
                reason: "ViewerEffect: Too many effects (max 255)".to_string()
            });
        }
        data.push(packet.effect.len() as u8); // Variable block count (1 byte)
        
        for effect in &packet.effect {
            // ID: LLUUID (16 bytes)
            data.extend_from_slice(effect.id.as_bytes());
            
            // AgentID: LLUUID (16 bytes) 
            data.extend_from_slice(effect.agent_id.as_bytes());
            
            // Type: U8 (1 byte)
            data.push(effect.r#type);
            
            // Duration: F32 (4 bytes, little-endian)
            data.write_f32::<LittleEndian>(effect.duration)
                .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write effect duration: {}", e) })?;
            
            // Color: Fixed 4 bytes (Color4U format: R,G,B,A)
            if effect.color.len() != 4 {
                return Err(NetworkError::PacketEncode {
                    reason: format!("ViewerEffect: Color must be exactly 4 bytes, got {}", effect.color.len())
                });
            }
            data.extend_from_slice(&effect.color);
            
            // TypeData: Variable1 (length byte + data)
            let type_data = &effect.type_data.data;
            if type_data.len() > 255 {
                return Err(NetworkError::PacketEncode {
                    reason: format!("ViewerEffect: TypeData too large (max 255 bytes), got {}", type_data.len())
                });
            }
            data.push(type_data.len() as u8);  // Variable1 length (1 byte)
            data.extend_from_slice(type_data);  // Variable1 data (n bytes)
        }
        
        Ok(())
    }

    /// Serialize AgentUpdate packet in proper SL format
    /// Based on message_template.msg: AgentData Single block with precise field layout
    fn serialize_agent_update(&self, packet: &AgentUpdate, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // AgentData single block - following exact template order:
        // AgentID (16 bytes), SessionID (16 bytes), BodyRotation (16 bytes), HeadRotation (16 bytes),
        // State (1 byte), CameraCenter (12 bytes), CameraAtAxis (12 bytes), CameraLeftAxis (12 bytes),
        // CameraUpAxis (12 bytes), Far (4 bytes), ControlFlags (4 bytes), Flags (1 byte)
        // Total: 16+16+16+16+1+12+12+12+12+4+4+1 = 122 bytes
        
        // AgentID: LLUUID (16 bytes)
        data.extend_from_slice(packet.agent_data.agent_id.as_bytes());
        
        // SessionID: LLUUID (16 bytes)  
        data.extend_from_slice(packet.agent_data.session_id.as_bytes());
        
        // BodyRotation: LLQuaternion (16 bytes: x,y,z,w as f32s - AgentUpdate uses FULL quaternions, not compressed)
        data.write_f32::<LittleEndian>(packet.agent_data.body_rotation.x)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write body rotation x: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.body_rotation.y)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write body rotation y: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.body_rotation.z)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write body rotation z: {}", e) })?;
        // Calculate and write W component: W = sqrt(1 - (X² + Y² + Z²))
        let x = packet.agent_data.body_rotation.x;
        let y = packet.agent_data.body_rotation.y; 
        let z = packet.agent_data.body_rotation.z;
        let w_squared = 1.0 - (x*x + y*y + z*z);
        let w = if w_squared > 0.0 { w_squared.sqrt() } else { 0.0 };
        data.write_f32::<LittleEndian>(w)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write body rotation w: {}", e) })?;
        
        // HeadRotation: LLQuaternion (16 bytes: x,y,z,w as f32s - AgentUpdate uses FULL quaternions, not compressed)
        data.write_f32::<LittleEndian>(packet.agent_data.head_rotation.x)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write head rotation x: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.head_rotation.y)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write head rotation y: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.head_rotation.z)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write head rotation z: {}", e) })?;
        // Calculate and write W component: W = sqrt(1 - (X² + Y² + Z²))
        let hx = packet.agent_data.head_rotation.x;
        let hy = packet.agent_data.head_rotation.y;
        let hz = packet.agent_data.head_rotation.z;
        let hw_squared = 1.0 - (hx*hx + hy*hy + hz*hz);
        let hw = if hw_squared > 0.0 { hw_squared.sqrt() } else { 0.0 };
        data.write_f32::<LittleEndian>(hw)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write head rotation w: {}", e) })?;
        
        // State: U8 (1 byte)
        data.push(packet.agent_data.state);
        
        // CameraCenter: LLVector3 (12 bytes: x,y,z as f32s)
        data.write_f32::<LittleEndian>(packet.agent_data.camera_center.x)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera center x: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_center.y)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera center y: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_center.z)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera center z: {}", e) })?;
        
        // CameraAtAxis: LLVector3 (12 bytes: x,y,z as f32s)
        data.write_f32::<LittleEndian>(packet.agent_data.camera_at_axis.x)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera at axis x: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_at_axis.y)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera at axis y: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_at_axis.z)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera at axis z: {}", e) })?;
        
        // CameraLeftAxis: LLVector3 (12 bytes: x,y,z as f32s)
        data.write_f32::<LittleEndian>(packet.agent_data.camera_left_axis.x)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera left axis x: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_left_axis.y)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera left axis y: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_left_axis.z)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera left axis z: {}", e) })?;
        
        // CameraUpAxis: LLVector3 (12 bytes: x,y,z as f32s)
        data.write_f32::<LittleEndian>(packet.agent_data.camera_up_axis.x)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera up axis x: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_up_axis.y)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera up axis y: {}", e) })?;
        data.write_f32::<LittleEndian>(packet.agent_data.camera_up_axis.z)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write camera up axis z: {}", e) })?;
        
        // Far: F32 (4 bytes)
        data.write_f32::<LittleEndian>(packet.agent_data.far)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write far: {}", e) })?;
        
        // ControlFlags: U32 (4 bytes, little-endian)
        data.write_u32::<LittleEndian>(packet.agent_data.control_flags)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write control flags: {}", e) })?;
        
        // Flags: U8 (1 byte)
        data.push(packet.agent_data.flags);
        
        // UnknownField1: U32 (4 bytes) - not in generated struct, use default
        data.write_u32::<LittleEndian>(0)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write unknown field 1: {}", e) })?;
        
        // UnknownField2: U32 (4 bytes) - not in generated struct, use default  
        data.write_u32::<LittleEndian>(0)
            .map_err(|e| NetworkError::PacketEncode { reason: format!("Failed to write unknown field 2: {}", e) })?;
        
        Ok(())
    }
    
    /// Serialize PacketAck packet in proper SL format
    /// PacketAck format: Variable number of U32 packet IDs to acknowledge
    fn serialize_packet_ack(&self, packet: &PacketAck, data: &mut Vec<u8>) -> NetworkResult<()> {
        use byteorder::{WriteBytesExt, LittleEndian};
        
        // PacketAck has Variable block "Packets" with field "ID" (U32)
        // Write number of acknowledgments as single byte (Variable block count)
        let num_acks = packet.packets.len() as u8;
        data.push(num_acks);
        
        // Write each packet ID to acknowledge (4 bytes each, little-endian)
        for ack_packet in &packet.packets {
            data.write_u32::<LittleEndian>(ack_packet.id)
                .map_err(|e| NetworkError::PacketEncode { 
                    reason: format!("Failed to write PacketAck ID: {}", e) 
                })?;
        }
        
        Ok(())
    }
    
    /// Serialize a packet wrapper (used for resends with same sequence)
    pub fn serialize_wrapper(&mut self, wrapper: &PacketWrapper) -> NetworkResult<Bytes> {
        let mut buffer = BytesMut::new();
        
        // Apply zerocoding if needed (check if original packet was zerocoded)
        let packet_info = crate::networking::packets::get_packet_info_by_id(
            wrapper.packet_id, 
            wrapper.frequency
        ).ok_or_else(|| NetworkError::PacketEncode {
            reason: format!("Unknown packet ID {} for frequency {:?}", 
                          wrapper.packet_id, wrapper.frequency)
        })?;
        
        let final_data = if packet_info.zerocoded {
            zerocode::encode(&wrapper.data)
        } else {
            wrapper.data.clone()
        };
        
        // Build header with wrapper's sequence and resent flag
        self.write_header_with_flags(&mut buffer, 
                                   wrapper.packet_id, 
                                   wrapper.frequency, 
                                   wrapper.reliable, 
                                   packet_info.zerocoded,
                                   wrapper.resent,
                                   wrapper.sequence);
        
        // Append packet data
        buffer.extend_from_slice(&final_data);
        
        Ok(buffer.freeze())
    }
    
    fn next_sequence(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence += 1;
        if self.sequence >= MAX_SEQUENCE {
            self.sequence = 1;
        }
        seq
    }
    
    /// Write Second Life packet header
    /// Format: [flags:1] [sequence:4] [extra:1] [message_id:1-4]
    fn write_header(&self, buffer: &mut BytesMut, 
                   packet_id: u16, 
                   frequency: PacketFrequency,
                   reliable: bool,
                   zerocoded: bool,
                   sequence: u32) {
        self.write_header_with_flags(buffer, packet_id, frequency, reliable, zerocoded, false, sequence);
    }
    
    /// Write Second Life packet header with explicit resent flag control
    /// Format: [flags:1] [sequence:4] [extra:1] [message_id:1-4]
    fn write_header_with_flags(&self, buffer: &mut BytesMut, 
                              packet_id: u16, 
                              frequency: PacketFrequency,
                              reliable: bool,
                              zerocoded: bool,
                              resent: bool,
                              sequence: u32) {
        // Flags byte
        let mut flags = 0u8;
        if reliable {
            flags |= ACK_FLAG;
        }
        if zerocoded {
            flags |= ZEROCODED_FLAG;
        }
        if resent {
            flags |= RESENT_FLAG;
        }
        buffer.put_u8(flags);
        
        // Sequence number (4 bytes, big-endian)
        buffer.put_u32(sequence);
        
        // Extra header byte (always 0)
        buffer.put_u8(0);
        
        // Message ID encoding based on frequency
        match frequency {
            PacketFrequency::High => {
                // High: single byte ID
                buffer.put_u8(packet_id as u8);
            }
            PacketFrequency::Medium => {
                // Medium: 0xFF + single byte ID
                buffer.put_u8(0xFF);
                buffer.put_u8(packet_id as u8);
            }
            PacketFrequency::Low => {
                // Low: 0xFF 0xFF + two byte ID (big-endian)
                buffer.put_u8(0xFF);
                buffer.put_u8(0xFF);
                buffer.put_u16(packet_id);
            }
            PacketFrequency::Fixed => {
                // Fixed: 0xFF 0xFF 0xFF + single byte ID
                buffer.put_u8(0xFF);
                buffer.put_u8(0xFF);
                buffer.put_u8(0xFF);
                buffer.put_u8(packet_id as u8);
            }
        }
    }
}

impl Default for PacketSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Packet deserializer that handles Second Life protocol format
pub struct PacketDeserializer {
    packet_registry: HashMap<u32, fn(&[u8]) -> NetworkResult<Box<dyn std::any::Any + Send + Sync>>>,
}

impl PacketDeserializer {
    pub fn new() -> Self {
        let mut deserializer = Self {
            packet_registry: HashMap::new(),
        };
        
        // Register known packet deserializers
        deserializer.register_all_packets();
        deserializer
    }
    
    /// Parse a raw UDP packet into a PacketWrapper
    pub fn parse_raw(&self, data: &[u8]) -> NetworkResult<PacketWrapper> {
        if data.len() < 6 {
            return Err(NetworkError::PacketDecode {
                reason: "Packet too short for header".to_string(),
            });
        }
        
        let mut buffer = PacketBuffer::new(data);
        
        // Parse header
        let flags = buffer.get_u8();
        let sequence = buffer.get_u32();
        let _extra = buffer.get_u8(); // Skip extra byte
        
        let reliable = (flags & ACK_FLAG) != 0;
        let zerocoded = (flags & ZEROCODED_FLAG) != 0;
        
        info!("Parsing packet: flags=0x{:02x}, sequence={}, reliable={}, zerocoded={}", 
               flags, sequence, reliable, zerocoded);
        
        // Parse message ID and determine frequency
        let (packet_id, frequency) = self.parse_message_id(&mut buffer)?;
        
        info!("Parsed message ID: {} ({:?})", packet_id, frequency);
        
        // Get remaining packet data
        let mut packet_data = buffer.remaining_bytes().to_vec();
        
        // Decode zerocoding if present
        if zerocoded {
            packet_data = zerocode::decode(&packet_data)?;
        }
        
        Ok(PacketWrapper {
            data: packet_data,
            reliable,
            resent: false, // Will be updated from packet header flags if needed
            sequence,
            packet_id,
            frequency,
            embedded_acks: None, // TODO: Parse embedded ACKs from packet header
        })
    }
    
    /// Parse message ID from buffer and return (id, frequency)
    fn parse_message_id(&self, buffer: &mut PacketBuffer) -> NetworkResult<(u16, PacketFrequency)> {
        let first_byte = buffer.get_u8();
        
        if first_byte != 0xFF {
            // High frequency: single byte
            return Ok((first_byte as u16, PacketFrequency::High));
        }
        
        let second_byte = buffer.get_u8();
        if second_byte != 0xFF {
            // Medium frequency: 0xFF + single byte
            return Ok((second_byte as u16, PacketFrequency::Medium));
        }
        
        let third_byte = buffer.get_u8();
        if third_byte != 0xFF {
            // Low frequency: 0xFF 0xFF + two bytes (big-endian)
            let fourth_byte = buffer.get_u8();
            // Big-endian: high byte first, then low byte
            let id = ((third_byte as u16) << 8) | (fourth_byte as u16);
            return Ok((id, PacketFrequency::Low));
        }
        
        // Fixed frequency: 0xFF 0xFF 0xFF + single byte
        let id = buffer.get_u8() as u16;
        Ok((id, PacketFrequency::Fixed))
    }
    
    /// Deserialize a PacketWrapper into a specific packet type
    pub fn deserialize<P: Packet>(&self, wrapper: &PacketWrapper) -> NetworkResult<P> {
        // Verify packet type matches
        if u32::from(wrapper.packet_id) != P::ID || wrapper.frequency != P::FREQUENCY {
            return Err(NetworkError::PacketDecode {
                reason: format!(
                    "Packet type mismatch: expected {}:{:?}, got {}:{:?}",
                    P::ID, P::FREQUENCY, wrapper.packet_id, wrapper.frequency
                ),
            });
        }
        
        // Try bincode first for backwards compatibility
        match bincode::deserialize(&wrapper.data) {
            Ok(packet) => Ok(packet),
            Err(e) => {
                warn!("Bincode deserialization failed for {}: {}, trying template-based parsing", 
                      P::name(), e);
                info!("📦 BINCODE FAILED for packet ID {} ({:?} frequency): {}", 
                      wrapper.packet_id, wrapper.frequency, e);
                
                // Fallback to template-based parsing
                self.deserialize_with_template(wrapper)
            }
        }
    }
    
    /// Deserialize using template-based parsing (fallback for when bincode fails)
    fn deserialize_with_template<P: Packet>(&self, wrapper: &PacketWrapper) -> NetworkResult<P> {
        use template_registry::get_message_template;
        use template_deserializer::TemplateDeserializer;
        
        // Get the message template for this packet type
        let template = get_message_template(wrapper.packet_id, wrapper.frequency)
            .ok_or_else(|| {
                warn!("🚨 NO TEMPLATE FOUND for packet ID {} ({:?} frequency)", 
                      wrapper.packet_id, wrapper.frequency);
                info!("📦 RAW PACKET DUMP (no template): {} bytes", wrapper.data.len());
                info!("📦 Raw packet bytes (first 64): {:02x?}", 
                      &wrapper.data[..std::cmp::min(64, wrapper.data.len())]);
                NetworkError::PacketDecode {
                    reason: format!("No message template found for packet {} ({:?})", 
                                   wrapper.packet_id, wrapper.frequency),
                }
            })?;
        
        // Parse the packet using the template
        let deserializer = TemplateDeserializer::new();
        let parsed_message = deserializer.parse_packet(&wrapper.data, template)?;
        
        // Convert parsed message to typed packet
        // For now, we'll only implement this for specific packet types that are causing issues
        self.convert_parsed_message_to_packet::<P>(parsed_message)
    }
    
    /// Convert a parsed message back to a typed packet struct
    fn convert_parsed_message_to_packet<P: Packet>(&self, parsed_message: template_deserializer::ParsedMessage) -> NetworkResult<P> {
        use crate::networking::packets::generated::*;
        use crate::networking::packets::types::*;
        
        // We need to implement conversion for each packet type that might have EOF issues
        // Start with LayerData since that's causing the most problems
        
        if P::name() == "LayerData" {
            // Convert parsed LayerData message to LayerData struct
            let layer_id_block = parsed_message.get_single_block("LayerID")?;
            let layer_data_block = parsed_message.get_single_block("LayerData")?;
            
            let layer_data = LayerData {
                layer_id: LayerDataLayerIDBlock {
                    r#type: layer_id_block.get_field("Type")?.as_u8()?,
                },
                layer_data: LayerDataLayerDataBlock {
                    data: LLVariable2 {
                        data: layer_data_block.get_field("Data")?.as_bytes()?.clone(),
                    },
                },
            };
            
            // This is a bit of a hack - we know this is LayerData so we can safely transmute
            // In a real implementation, we'd use an enum or trait object approach
            let packet_any = Box::new(layer_data) as Box<dyn Any>;
            match packet_any.downcast::<P>() {
                Ok(packet) => Ok(*packet),
                Err(_) => Err(NetworkError::PacketDecode {
                    reason: format!("Failed to downcast parsed LayerData to {}", P::name()),
                }),
            }
        } else if P::name() == "AgentUpdate" {
            // Convert parsed AgentUpdate message to AgentUpdate struct
            // The template-based parser will handle extra fields gracefully
            let agent_data_block = parsed_message.get_single_block("AgentData")?;
            
            use crate::networking::packets::types::*;
            
            let agent_update = AgentUpdate {
                agent_data: AgentUpdateAgentDataBlock {
                    agent_id: {
                        let uuid_bytes = agent_data_block.get_field("AgentID")?.as_uuid_bytes()?;
                        LLUUID::from_bytes(uuid_bytes)
                    },
                    session_id: {
                        let uuid_bytes = agent_data_block.get_field("SessionID")?.as_uuid_bytes()?;
                        LLUUID::from_bytes(uuid_bytes)
                    },
                    body_rotation: {
                        let quat_data = agent_data_block.get_field("BodyRotation")?.as_quaternion()?;
                        LLQuaternion { x: quat_data[0], y: quat_data[1], z: quat_data[2] }
                    },
                    head_rotation: {
                        let quat_data = agent_data_block.get_field("HeadRotation")?.as_quaternion()?;
                        LLQuaternion { x: quat_data[0], y: quat_data[1], z: quat_data[2] }
                    },
                    state: agent_data_block.get_field("State")?.as_u8()?,
                    camera_center: {
                        let vec_data = agent_data_block.get_field("CameraCenter")?.as_vector3()?;
                        LLVector3 { x: vec_data[0], y: vec_data[1], z: vec_data[2] }
                    },
                    camera_at_axis: {
                        let vec_data = agent_data_block.get_field("CameraAtAxis")?.as_vector3()?;
                        LLVector3 { x: vec_data[0], y: vec_data[1], z: vec_data[2] }
                    },
                    camera_left_axis: {
                        let vec_data = agent_data_block.get_field("CameraLeftAxis")?.as_vector3()?;
                        LLVector3 { x: vec_data[0], y: vec_data[1], z: vec_data[2] }
                    },
                    camera_up_axis: {
                        let vec_data = agent_data_block.get_field("CameraUpAxis")?.as_vector3()?;
                        LLVector3 { x: vec_data[0], y: vec_data[1], z: vec_data[2] }
                    },
                    far: agent_data_block.get_field("Far")?.as_f32()?,
                    control_flags: agent_data_block.get_field("ControlFlags")?.as_u32()?,
                    flags: agent_data_block.get_field("Flags")?.as_u8()?,
                },
            };
            
            let packet_any = Box::new(agent_update) as Box<dyn Any>;
            match packet_any.downcast::<P>() {
                Ok(packet) => Ok(*packet),
                Err(_) => Err(NetworkError::PacketDecode {
                    reason: format!("Failed to downcast parsed AgentUpdate to {}", P::name()),
                }),
            }
        } else if P::name() == "CoarseLocationUpdate" {
            // Convert parsed CoarseLocationUpdate message
            let index_block = parsed_message.get_single_block("Index")?;
            let empty_location_blocks = vec![];
            let empty_agent_data_blocks = vec![];
            let location_blocks = parsed_message.blocks.get("Location").unwrap_or(&empty_location_blocks);
            let agent_data_blocks = parsed_message.blocks.get("AgentData").unwrap_or(&empty_agent_data_blocks);
            
            use crate::networking::packets::generated::CoarseLocationUpdate;
            use crate::networking::packets::generated::*;
            
            let coarse_update = CoarseLocationUpdate {
                location: location_blocks.iter().map(|block| {
                    CoarseLocationUpdateLocationBlock {
                        x: block.get_field("X").and_then(|f| f.as_u8()).unwrap_or(0),
                        y: block.get_field("Y").and_then(|f| f.as_u8()).unwrap_or(0),
                        z: block.get_field("Z").and_then(|f| f.as_u8()).unwrap_or(0),
                    }
                }).collect(),
                index: CoarseLocationUpdateIndexBlock {
                    you: index_block.get_field("You")?.as_s16().unwrap_or(-1),
                    prey: index_block.get_field("Prey")?.as_s16().unwrap_or(-1),
                },
                agent_data: agent_data_blocks.iter().map(|block| {
                    CoarseLocationUpdateAgentDataBlock {
                        agent_id: {
                            let uuid_bytes = block.get_field("AgentID").and_then(|f| f.as_uuid_bytes()).unwrap_or([0u8; 16]);
                            LLUUID::from_bytes(uuid_bytes)
                        },
                    }
                }).collect(),
            };
            
            let packet_any = Box::new(coarse_update) as Box<dyn Any>;
            match packet_any.downcast::<P>() {
                Ok(packet) => Ok(*packet),
                Err(_) => Err(NetworkError::PacketDecode {
                    reason: format!("Failed to downcast parsed CoarseLocationUpdate to {}", P::name()),
                }),
            }
        } else if P::name() == "SimulatorViewerTimeMessage" {
            // Convert parsed SimulatorViewerTimeMessage
            let time_info_block = parsed_message.get_single_block("TimeInfo")?;
            
            use crate::networking::packets::generated::SimulatorViewerTimeMessage;
            use crate::networking::packets::generated::*;
            
            let time_message = SimulatorViewerTimeMessage {
                time_info: SimulatorViewerTimeMessageTimeInfoBlock {
                    usec_since_start: time_info_block.get_field("UsecSinceStart")?.as_u64().unwrap_or(0),
                    sec_per_day: time_info_block.get_field("SecPerDay")?.as_u32().unwrap_or(86400),
                    sec_per_year: time_info_block.get_field("SecPerYear")?.as_u32().unwrap_or(31536000),
                    sun_direction: {
                        let vec_data = time_info_block.get_field("SunDirection")?.as_vector3()?;
                        LLVector3 { x: vec_data[0], y: vec_data[1], z: vec_data[2] }
                    },
                    sun_phase: time_info_block.get_field("SunPhase")?.as_f32().unwrap_or(0.0),
                    sun_ang_velocity: {
                        let vec_data = time_info_block.get_field("SunAngVelocity")?.as_vector3()?;
                        LLVector3 { x: vec_data[0], y: vec_data[1], z: vec_data[2] }
                    },
                },
            };
            
            let packet_any = Box::new(time_message) as Box<dyn Any>;
            match packet_any.downcast::<P>() {
                Ok(packet) => Ok(*packet),
                Err(_) => Err(NetworkError::PacketDecode {
                    reason: format!("Failed to downcast parsed SimulatorViewerTimeMessage to {}", P::name()),
                }),
            }
        } else {
            // For other packet types, log the raw packet data and template info
            warn!("Template-based deserialization not implemented for packet type: {}", P::name());
            info!("📦 RAW PACKET DUMP for {}", P::name());
            info!("📦 Template name: {}", parsed_message.name);
            info!("📦 Template blocks found: {:?}", parsed_message.blocks.keys().collect::<Vec<_>>());
            
            // Also log each block's fields for debugging
            for (block_name, blocks) in &parsed_message.blocks {
                info!("📦   Block '{}': {} instances", block_name, blocks.len());
                for (i, block) in blocks.iter().enumerate() {
                    info!("📦     Instance {}: fields = {:?}", i, block.fields.keys().collect::<Vec<_>>());
                    for (field_name, field_value) in &block.fields {
                        info!("📦       {}: {:?}", field_name, field_value);
                    }
                }
            }
            
            // Try bincode as last resort, but don't default to AttachedSound
            bincode::deserialize(&[])
                .map_err(|e| NetworkError::PacketDecode {
                    reason: format!("No deserialization method available for packet type {}: {}", P::name(), e),
                })
        }
    }
    
    fn register_all_packets(&mut self) {
        use crate::networking::packets::generated::*;
        
        // Register deserializers for all known packets
        self.register_packet::<UseCircuitCode>();
        self.register_packet::<CompleteAgentMovement>();
        self.register_packet::<RegionHandshake>();
        self.register_packet::<RegionHandshakeReply>();
        self.register_packet::<AgentThrottle>();
        self.register_packet::<AgentUpdate>();
        self.register_packet::<AgentHeightWidth>();
        self.register_packet::<LogoutRequest>();
        self.register_packet::<PacketAck>();
        self.register_packet::<SimulatorViewerTimeMessage>();
        self.register_packet::<LayerData>();
        self.register_packet::<CoarseLocationUpdate>();
    }
    
    fn register_packet<P: Packet + 'static>(&mut self) {
        let key = P::lookup_key();
        self.packet_registry.insert(key, |data| {
            let packet: P = bincode::deserialize(data)
                .map_err(|e| crate::networking::NetworkError::PacketDecode {
                    reason: e.to_string()
                })?;
            Ok(Box::new(packet))
        });
    }
}

impl Default for PacketDeserializer {
    fn default() -> Self {
        Self::new()
    }
}