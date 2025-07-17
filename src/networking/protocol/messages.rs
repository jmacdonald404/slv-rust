use serde::{Serialize, Deserialize};
use bincode::{Encode, Decode};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
pub struct PacketHeader {
    pub sequence_id: u32,
    pub flags: u8,
}

#[derive(Debug, Clone)]
pub struct RegionHandshakeData {
    pub region_flags: u32,
    pub sim_access: u8,
    pub region_name: String,
    pub sim_owner: Uuid,
    pub is_estate_manager: u8,
    pub water_height: f32,
    pub billable_factor: f32,
    pub cache_id: Uuid,
    pub terrain_base: [Uuid; 4],
    pub terrain_detail: [Uuid; 4],
    pub terrain_start_height: [f32; 4],
    pub terrain_height_range: [f32; 4],
    pub region_id: Uuid,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Placeholder for various Second Life messages
    KeepAlive,
    Logout,
    Ack { sequence_id: u32 },
    // IMPORTANT NOTE: UseCircuitCode Packet Structure
    // This message variant defines the canonical structure for the UseCircuitCode packet.
    // It includes agent_id (String, derived from Uuid), session_id (String, derived from Uuid),
    // and circuit_code (u32). This structure is a critical source of truth for how
    // new circuit codes are communicated and should be referenced for any related
    // encoding/decoding or message additions.
    UseCircuitCode {
        agent_id: String,
        session_id: String,
        circuit_code: u32,
    },
    UseCircuitCodeReply(bool),
    // Chat message from the viewer to the simulator
    ChatFromViewer {
        message: String,
        channel: String, // e.g., "local", "IM", "group" (stub for now)
    },
    // Chat message from the simulator to the viewer
    ChatFromSimulator {
        sender: String,
        message: String,
        channel: String, // e.g., "local", "IM", "group" (stub for now)
    },
    // Add more message types as needed
    // IMPORTANT NOTE: CompleteAgentMovement Packet Structure
    // This message variant defines the canonical structure for the CompleteAgentMovement packet.
    // It includes agent_id (String, derived from Uuid), session_id (String, derived from Uuid),
    // circuit_code (u32), position ((f32, f32, f32)), and look_at ((f32, f32, f32)).
    // This structure is a critical source of truth for agent position and orientation updates
    // and should be referenced for any related encoding/decoding or message additions.
    CompleteAgentMovement {
        agent_id: String,
        session_id: String,
        circuit_code: u32,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
    },
    AgentUpdate {
        agent_id: String,
        session_id: String,
        position: (f32, f32, f32),
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        controls: u32,
    },
    AgentMovementComplete {
        agent_id: String,
        session_id: String,
        // TODO: Add position, region info, etc.
    },
    RegionHandshake(RegionHandshakeData),
    RegionHandshakeReply {
        agent_id: String,
        session_id: String,
        flags: u32,
    },
    AgentThrottle {
        agent_id: String,
        session_id: String,
        circuit_code: u32,
        throttle: [f32; 7],
    },
    AgentDataUpdate {
        agent_id: String,
        // Placeholder for actual fields
    },
    HealthMessage {
        // Placeholder for actual fields
    },
}
