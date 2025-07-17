use crate::networking::protocol::region_handshake::parse_region_handshake;
use crate::networking::protocol::messages::{PacketHeader, Message, RegionHandshakeData};
use std::io::{self, ErrorKind};
use uuid::Uuid;

pub struct MessageCodec;

impl MessageCodec {
    /// Manually decode LLUDP messages. Supports RegionHandshake, ACK, AgentMovementComplete, KeepAlive.
    pub fn decode(data: &[u8]) -> io::Result<(PacketHeader, Message)> {
        // RegionHandshake (0xFF, 0xFF, 0x00, 0x03)
        if data.len() > 10 && data[6..10] == [0xFF, 0xFF, 0x00, 0x03] {
            println!("[CODEC] Parsed RegionHandshake");
            let header = PacketHeader { sequence_id: 0, flags: data[0] };
            let payload = &data[10..];
            if let Some(rh) = parse_region_handshake(payload) {
                return Ok((header, Message::RegionHandshake(rh)));
            } else {
                return Err(io::Error::new(ErrorKind::InvalidData, "Failed to parse RegionHandshake"));
            }
        }
        // ACK (0x00, 0x00, 0x00, 0x01)
        if data.len() > 14 && data[6..10] == [0x00, 0x00, 0x00, 0x01] {
            println!("[CODEC] Parsed ACK");
            let header = PacketHeader { sequence_id: 0, flags: data[0] };
            let acked_seq = u32::from_be_bytes([data[10], data[11], data[12], data[13]]);
            return Ok((header, Message::Ack { sequence_id: acked_seq }));
        }
        // AgentMovementComplete (0xFF, 0xFF, 0x00, 0xF9)
        if data.len() > 42 && data[6..10] == [0xFF, 0xFF, 0x00, 0xF9] {
            println!("[CODEC] Parsed AgentMovementComplete");
            let header = PacketHeader { sequence_id: 0, flags: data[0] };
            let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
            let session_id = Uuid::from_slice(&data[26..42]).map(|u| u.to_string()).unwrap_or_default();
            return Ok((header, Message::AgentMovementComplete { agent_id, session_id }));
        }
        // ImprovedAvatarPowers (0xFF, 0xFF, 0x00, 0xFA)
        if data.len() > 34 && data[6..10] == [0xFF, 0xFF, 0x00, 0xFA] {
            println!("[CODEC] Parsed ImprovedAvatarPowers");
            let header = PacketHeader { sequence_id: 0, flags: data[0] };
            let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
            let powers = u64::from_be_bytes([
                data[26], data[27], data[28], data[29], data[30], data[31], data[32], data[33]
            ]);
            // You may want to add a Message::ImprovedAvatarPowers { agent_id, powers } variant
            // For now, just log and return an error or add a stub variant
            return Err(io::Error::new(ErrorKind::InvalidData, "ImprovedAvatarPowers not handled"));
        }
        // KeepAlive (0xFF, 0xFF, 0xFF, 0xFB)
        if data.len() > 10 && data[6..10] == [0xFF, 0xFF, 0xFF, 0xFB] {
            println!("[CODEC] Parsed KeepAlive");
            let header = PacketHeader { sequence_id: 0, flags: data[0] };
            return Ok((header, Message::KeepAlive));
        }
        // StartPingCheck (0xFF, 0xFF, 0x00, 0x01)
        if data.len() > 14 && data[6..10] == [0xFF, 0xFF, 0x00, 0x01] {
            println!("[CODEC] Parsed StartPingCheck");
            let header = PacketHeader { sequence_id: 0, flags: data[0] };
            let ping_id = u32::from_be_bytes([data[10], data[11], data[12], data[13]]);
            // You may want to add a Message::StartPingCheck { ping_id } variant
            // For now, just log and return an error or add a stub variant
            return Err(io::Error::new(ErrorKind::InvalidData, "StartPingCheck not handled"));
        }
        // ImprovedTerseObjectUpdate
        if data.len() > 10 && data[6..10] == [0xFF, 0xFF, 0x01, 0x83] {
            println!("[CODEC] Received ImprovedTerseObjectUpdate (not parsed)");
            // Optionally: return Ok((header, Message::Unknown)); // if you add an Unknown variant
            return Err(io::Error::new(ErrorKind::InvalidData, "ImprovedTerseObjectUpdate not parsed"));
        }
        println!("[CODEC] Unknown or unsupported message: {:02X?}", &data[..std::cmp::min(data.len(), 32)]);
        Err(io::Error::new(ErrorKind::InvalidData, "Unsupported or unknown message type"))
    }
}
