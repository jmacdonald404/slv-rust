use serde::{Serialize, Deserialize};
use bincode::{Encode, Decode};

#[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
pub struct PacketHeader {
    pub sequence_id: u32,
    pub flags: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
pub enum Message {
    // Placeholder for various Second Life messages
    KeepAlive,
    Logout,
    Ack { sequence_id: u32 },
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
}
