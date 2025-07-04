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
}
