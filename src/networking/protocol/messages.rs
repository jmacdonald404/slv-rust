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
    // Add more message types as needed
}
