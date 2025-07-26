use crate::networking::protocol::region_handshake::parse_region_handshake;
use crate::networking::protocol::messages::{PacketHeader, Message, RegionHandshakeData};
use std::io::{self, ErrorKind};
use uuid::Uuid;

pub struct MessageCodec;

impl MessageCodec {
    /// Manually decode LLUDP messages. Supports RegionHandshake, ACK, AgentMovementComplete, KeepAlive.
    pub fn decode(data: &[u8]) -> io::Result<(PacketHeader, Message)> {
        if data.len() < 7 {
            println!("[CODEC] Packet too short: {} bytes", data.len());
            return Err(io::Error::new(ErrorKind::InvalidData, "Packet too short for message ID"));
        }

        let header = PacketHeader {
            sequence_id: u32::from_be_bytes(data[1..5].try_into().unwrap_or_default()),
            flags: data[0],
        };

        // Message IDs can be 1, 2, or 4 bytes.
        // High frequency = 1 byte. Medium = 2 bytes. Low = 4 bytes.
        // The first two bytes of 2 and 4-byte IDs are 0xFF.
        
        let id_byte1 = data[6];
        
        // --- High Frequency Messages ---
        if id_byte1 < 0xFF {
            match id_byte1 {
                5 => { // RegionHandshake
                    println!("[CODEC] Parsed RegionHandshake");
                    let payload = &data[7..];
                    return if let Some(rh) = parse_region_handshake(payload) {
                        Ok((header, Message::RegionHandshake(rh)))
                    } else {
                        Err(io::Error::new(ErrorKind::InvalidData, "Failed to parse RegionHandshake"))
                    };
                },
                _ => {
                    // Other high-frequency messages can be added here.
                }
            }
        }

        // --- Medium and Low Frequency Messages ---
        if data.len() >= 10 {
            let full_id = &data[6..10];
            match full_id {
                [0xFF, 0xFF, 0xFF, 0xFB] => { // KeepAlive
                    println!("[CODEC] Parsed KeepAlive");
                    return Ok((header, Message::KeepAlive));
                },
                // IMPORTANT NOTE: AgentMovementComplete Packet Structure
                // This decoding logic for AgentMovementComplete is a critical source of truth.
                // It defines the exact byte offsets and data types for agent_id (16 bytes, Uuid)
                // and session_id (16 bytes, Uuid) within the packet payload.
                // Any future modifications or new message additions that relate to agent movement
                // or session management MUST refer to this structure to ensure compatibility and correctness.
                // The packet is expected to be at least 42 bytes long to contain these UUIDs.
                [0xFF, 0xFF, 0x00, 0xF9] => { // AgentMovementComplete
                    println!("[CODEC] Parsed AgentMovementComplete");
                    if data.len() >= 42 {
                        let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
                        let session_id = Uuid::from_slice(&data[26..42]).map(|u| u.to_string()).unwrap_or_default();
                        return Ok((header, Message::AgentMovementComplete { agent_id, session_id }));
                    } else {
                        return Err(io::Error::new(ErrorKind::InvalidData, "Packet too short for AgentMovementComplete"));
                    }
                },
                [0xFF, 0xFF, 0x00, 0xFA] => { // ImprovedAvatarPowers
                     println!("[CODEC] Parsed ImprovedAvatarPowers");
                     if data.len() >= 34 {
                        let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
                        let powers = u64::from_le_bytes(data[26..34].try_into().unwrap_or_default());
                        // Message::ImprovedAvatarPowers variant would be needed
                        return Err(io::Error::new(ErrorKind::InvalidData, "ImprovedAvatarPowers not fully handled"));
                     } else {
                        return Err(io::Error::new(ErrorKind::InvalidData, "Packet too short for ImprovedAvatarPowers"));
                     }
                },
                [0xFF, 0xFF, 0x00, 0x01] => { // StartPingCheck
                    println!("[CODEC] Parsed StartPingCheck");
                    if data.len() >= 14 {
                        let _ping_id = u32::from_le_bytes(data[10..14].try_into().unwrap_or_default());
                        // Message::StartPingCheck variant would be needed
                        return Err(io::Error::new(ErrorKind::InvalidData, "StartPingCheck not fully handled"));
                    } else {
                        return Err(io::Error::new(ErrorKind::InvalidData, "Packet too short for StartPingCheck"));
                    }
                },
                [0xFF, 0xFF, 0x01, 0x83] => { // AgentDataUpdate
                    println!("[CODEC] Parsed AgentDataUpdate");
                    if data.len() >= 26 {
                        let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
                        return Ok((header, Message::AgentDataUpdate{ agent_id }));
                    } else {
                        return Err(io::Error::new(ErrorKind::InvalidData, "Packet too short for AgentDataUpdate"));
                    }
                },
                [0xFF, 0xFF, 0x00, 0x8A] => { // HealthMessage
                    println!("[CODEC] Parsed HealthMessage");
                    // Placeholder for actual parsing
                    return Ok((header, Message::HealthMessage{}));
                },
                _ => {} // Fall through to unknown
            }
        }
        
        // --- ACK ---
        // This is a special case. It's identified by a flag in the header, not a message ID in the body.
        // The body of an ACK-only packet contains a list of acknowledged sequence numbers.
        if header.flags & 0x10 != 0 { // ACK_FLAG
            println!("[CODEC] Parsed ACK");
            if data.len() >= 10 { // At least one ACK
                let acked_seq = u32::from_be_bytes(data[6..10].try_into().unwrap_or_default());
                return Ok((header, Message::Ack { sequence_id: acked_seq }));
            }
        }

        println!("[CODEC] Unknown or unsupported message: {:02X?}", &data[..std::cmp::min(data.len(), 32)]);
        Err(io::Error::new(ErrorKind::InvalidData, "Unsupported or unknown message type"))
    }

    /// Encode a message with the given header into a binary packet
    pub fn encode(header: &PacketHeader, message: &Message) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        
        // Add flags and sequence ID
        buf.push(header.flags);
        buf.extend_from_slice(&header.sequence_id.to_be_bytes());
        buf.push(0x00); // offset, always 0
        
        match message {
            Message::UseCircuitCode { agent_id, session_id, circuit_code } => {
                // Low frequency message ID for UseCircuitCode (3)
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x03]);
                buf.extend_from_slice(&circuit_code.to_le_bytes());
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(agent_uuid.as_bytes());
            },
            Message::CompleteAgentMovement { agent_id, session_id, circuit_code, .. } => {
                // Low frequency message ID for CompleteAgentMovement (249)
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0xF9]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(&circuit_code.to_le_bytes());
            },
            Message::RegionHandshakeReply { agent_id, session_id, flags } => {
                // High frequency message ID for RegionHandshakeReply (6)
                buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x06]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(&flags.to_le_bytes());
            },
            Message::AgentThrottle { agent_id, session_id, circuit_code, throttle } => {
                // Low frequency message ID for AgentThrottle (81)
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x51]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(&circuit_code.to_be_bytes());
                for v in throttle.iter() {
                    buf.extend_from_slice(&v.to_be_bytes());
                }
            },
            Message::AgentUpdate { agent_id, session_id, position, camera_at, camera_eye, controls } => {
                // High frequency message ID for AgentUpdate (4)
                buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x04]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                let coords = [position.0, position.1, position.2, camera_at.0, camera_at.1, camera_at.2, camera_eye.0, camera_eye.1, camera_eye.2];
                for v in coords.iter() {
                    buf.extend_from_slice(&v.to_be_bytes());
                }
                buf.extend_from_slice(&controls.to_be_bytes());
            },
            Message::Ack { sequence_id } => {
                // ACK message (1)
                buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                buf.extend_from_slice(&sequence_id.to_be_bytes());
            },
            _ => {
                return Err(io::Error::new(ErrorKind::InvalidInput, "Message type not supported for encoding"));
            }
        }
        
        Ok(buf)
    }
}
