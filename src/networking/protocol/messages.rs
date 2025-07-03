use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PacketHeader {
    pub sequence_id: u32,
    pub flags: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    // Placeholder for various Second Life messages
    KeepAlive,
    Logout,
    // Add more message types as needed
}
