//! Generated packet definitions for Second Life protocol
//! 
//! These packets are generated to match the exact format expected by Second Life simulators,
//! ensuring 100% protocol compatibility while providing Rust's type safety.

use super::{Packet, PacketFrequency};
use super::types::*;
use serde::{Deserialize, Serialize};

/// UseCircuitCode packet - establishes circuit with simulator
/// This is the first packet sent to establish a UDP connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCircuitCode {
    pub circuit_code: CircuitCodeBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitCodeBlock {
    pub code: U32,
    pub session_id: LLUUID,
    pub id: LLUUID,
}

impl Packet for UseCircuitCode {
    const ID: u16 = 3;
    const FREQUENCY: PacketFrequency = PacketFrequency::Fixed;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = true;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "UseCircuitCode" }
}

/// CompleteAgentMovement packet - completes avatar initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteAgentMovement {
    pub agent_data: AgentDataBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDataBlock {
    pub agent_id: LLUUID,
    pub session_id: LLUUID,
    pub circuit_code: U32,
}

impl Packet for CompleteAgentMovement {
    const ID: u16 = 249;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = true;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "CompleteAgentMovement" }
}

/// RegionHandshake packet - server sends region information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionHandshake {
    pub region_info: RegionInfoBlock,
    pub region_info2: RegionInfo2Block,
    pub region_info3: RegionInfo3Block,
    pub region_info4: RegionInfo4Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfoBlock {
    pub region_flags: U32,
    pub sim_access: U8,
    pub sim_name: LLVariable1,
    pub sim_owner: LLUUID,
    pub is_estate_manager: BOOL,
    pub water_height: F32,
    pub billable_factor: F32,
    pub cache_id: LLUUID,
    pub terrain_raise_limit: F32,
    pub terrain_lower_limit: F32,
    pub price_per_meter: S32,
    pub redirect_grid_x: S32,
    pub redirect_grid_y: S32,
    pub use_estate_sun: BOOL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo2Block {
    pub region_id: LLUUID,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo3Block {
    pub cpu_class_id: S32,
    pub cpu_ratio: S32,
    pub colocated_simulator: LLVariable1,
    pub product_sku: LLVariable1,
    pub product_name: LLVariable1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo4Block {
    pub region_flags_extended: U64,
    pub region_protocols: U64,
}


impl Packet for RegionHandshake {
    const ID: u16 = 148;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = true;
    const TRUSTED: bool = true;
    
    fn name() -> &'static str { "RegionHandshake" }
}

/// RegionHandshakeReply packet - client acknowledges region handshake
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionHandshakeReply {
    pub agent_data: AgentDataBlock,
    pub region_info: RegionHandshakeReplyRegionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionHandshakeReplyRegionInfo {
    pub flags: U32,
}

impl Packet for RegionHandshakeReply {
    const ID: u16 = 149;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = false;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "RegionHandshakeReply" }
}

/// AgentThrottle packet - sets bandwidth throttles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThrottle {
    pub agent_data: AgentDataBlock,
    pub throttle: ThrottleBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleBlock {
    pub gen_counter: U32,
    pub throttles: LLVariable1, // 7 floats: resend, land, wind, cloud, task, texture, asset
}

impl Packet for AgentThrottle {
    const ID: u16 = 251;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = false;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "AgentThrottle" }
}

/// AgentUpdate packet - sends avatar movement and camera updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdate {
    pub agent_data: AgentUpdateDataBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdateDataBlock {
    pub agent_id: LLUUID,
    pub session_id: LLUUID,
    pub body_rotation: LLQuaternion,
    pub head_rotation: LLQuaternion,
    pub state: U8,
    pub camera_center: LLVector3,
    pub camera_at_axis: LLVector3,
    pub camera_left_axis: LLVector3,
    pub camera_up_axis: LLVector3,
    pub far: F32,
    pub control_flags: U32,
    pub flags: U8,
}

impl Packet for AgentUpdate {
    const ID: u16 = 4;
    const FREQUENCY: PacketFrequency = PacketFrequency::High;
    const RELIABLE: bool = false;
    const ZEROCODED: bool = false;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "AgentUpdate" }
}

/// AgentHeightWidth packet - sends avatar height and width
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeightWidth {
    pub agent_data: AgentDataBlock,
    pub height_width_block: HeightWidthBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightWidthBlock {
    pub gen_counter: U32,
    pub height: U16,
    pub width: U16,
}

impl Packet for AgentHeightWidth {
    const ID: u16 = 250;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = false;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "AgentHeightWidth" }
}

/// LogoutRequest packet - requests logout from simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutRequest {
    pub agent_data: AgentDataBlock,
}

impl Packet for LogoutRequest {
    const ID: u16 = 252;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = false;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "LogoutRequest" }
}

/// PacketAck packet - acknowledges reliable packets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketAck {
    pub packets: Vec<PacketAckPacketsBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketAckPacketsBlock {
    pub id: U32,
}

impl Packet for PacketAck {
    const ID: u16 = 251;
    const FREQUENCY: PacketFrequency = PacketFrequency::Fixed;
    const RELIABLE: bool = false;
    const ZEROCODED: bool = false;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "PacketAck" }
}

/// ChatFromViewer packet - sends chat messages to simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatFromViewer {
    pub agent_data: AgentDataBlock,
    pub chat_data: ChatDataBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDataBlock {
    pub message: LLVariable1,
    pub chat_type: U8,
    pub channel: U32,
}

impl Packet for ChatFromViewer {
    const ID: u16 = 80;
    const FREQUENCY: PacketFrequency = PacketFrequency::Low;
    const RELIABLE: bool = true;
    const ZEROCODED: bool = true;
    const TRUSTED: bool = false;
    
    fn name() -> &'static str { "ChatFromViewer" }
}