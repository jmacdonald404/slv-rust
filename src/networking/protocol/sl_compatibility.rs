// Compatibility layer between auto-generated messages and SL handshake logic
use super::codecs::PacketHeader;
use std::io;
use uuid::Uuid;

/// Simplified message types for handshake state machine
#[derive(Debug, Clone)]
pub enum HandshakeMessage {
    UseCircuitCode {
        agent_id: String,
        session_id: String, 
        circuit_code: u32,
    },
    UseCircuitCodeReply(bool),
    CompleteAgentMovement {
        agent_id: String,
        session_id: String,
        circuit_code: u32,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
    },
    AgentMovementComplete {
        agent_id: String,
        session_id: String,
    },
    RegionHandshake {
        region_name: String,
        region_id: Uuid,
        region_flags: u32,
        water_height: f32,
        sim_access: u8,
    },
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
    AgentUpdate {
        agent_id: String,
        session_id: String,
        position: (f32, f32, f32),
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        controls: u32,
    },
    AgentDataUpdate {
        agent_id: String,
    },
    HealthMessage,
    KeepAlive,
    Ack {
        sequence_id: u32,
    },
    ChatFromViewer {
        message: String,
        channel: String,
    },
    ChatFromSimulator {
        sender: String,
        message: String,
        channel: String,
    },
    OnlineNotification {
        agent_ids: Vec<String>,
    },
    Logout,
}

pub struct SLMessageCodec;

impl SLMessageCodec {
    /// Encode a handshake message using SL protocol rules
    pub fn encode_handshake(header: &PacketHeader, message: &HandshakeMessage) -> io::Result<Vec<u8>> {
        tracing::info!("[SL_CODEC] 🏗️  Encoding message: {:?} with header seq={}, flags=0x{:02X}", message, header.sequence_id, header.flags);
        let mut buf = Vec::new();
        
        // Add flags and sequence ID
        buf.push(header.flags);
        buf.extend_from_slice(&header.sequence_id.to_be_bytes());
        buf.push(0x00); // offset, always 0
        
        match message {
            HandshakeMessage::UseCircuitCode { agent_id, session_id, circuit_code } => {
                // Low frequency message ID for UseCircuitCode (3)
                tracing::info!("[SL_CODEC] 📤 UseCircuitCode: Low frequency [0xFF, 0xFF, 0x00, 0x03], circuit_code={}", circuit_code);
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x03]);
                buf.extend_from_slice(&circuit_code.to_le_bytes());
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(agent_uuid.as_bytes());
            },
            HandshakeMessage::CompleteAgentMovement { agent_id, session_id, circuit_code, .. } => {
                // Low frequency message ID for CompleteAgentMovement (249)
                tracing::info!("[SL_CODEC] 📤 CompleteAgentMovement: Low frequency [0xFF, 0xFF, 0x00, 0xF9], circuit_code={}", circuit_code);
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0xF9]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(&circuit_code.to_le_bytes());
            },
            HandshakeMessage::RegionHandshakeReply { agent_id, session_id, flags } => {
                // Low frequency message ID for RegionHandshakeReply (149 = 0x95)
                tracing::info!("[SL_CODEC] 📤 RegionHandshakeReply: Low frequency [0xFF,0xFF,0x00,0x95], flags={}", flags);
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x95]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(&flags.to_le_bytes());
            },
            HandshakeMessage::AgentThrottle { agent_id, session_id, circuit_code, throttle } => {
                // Low frequency message ID for AgentThrottle (81)
                buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x51]);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                buf.extend_from_slice(&circuit_code.to_be_bytes());
                for v in throttle.iter() {
                    buf.extend_from_slice(&v.to_be_bytes());
                }
            },
            HandshakeMessage::AgentUpdate { agent_id, session_id, position, camera_at, camera_eye, controls } => {
                // High frequency message ID for AgentUpdate (4) - single byte
                buf.push(0x04);
                let agent_uuid = Uuid::parse_str(agent_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid agent_id UUID"))?;
                let session_uuid = Uuid::parse_str(session_id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid session_id UUID"))?;
                buf.extend_from_slice(agent_uuid.as_bytes());
                buf.extend_from_slice(session_uuid.as_bytes());
                let coords = [position.0, position.1, position.2, camera_at.0, camera_at.1, camera_at.2, camera_eye.0, camera_eye.1, camera_eye.2];
                for v in coords.iter() {
                    buf.extend_from_slice(&v.to_be_bytes());
                }
                buf.extend_from_slice(&controls.to_be_bytes());
            },
            HandshakeMessage::Ack { sequence_id } => {
                // PacketAck (0xFFFFFFFB) - Fixed frequency message
                tracing::info!("[SL_CODEC] 📤 PacketAck: Fixed frequency [0xFF,0xFF,0xFF,0xFB], acking sequence={}", sequence_id);
                buf.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFB]);
                // PacketAck has a Variable "Packets" block with U32 ID fields
                buf.push(1); // Number of packet IDs in the variable block
                buf.extend_from_slice(&sequence_id.to_be_bytes());
            },
            _ => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Message type not supported for encoding"));
            }
        }
        
        tracing::info!("[SL_CODEC] ✅ Encoded {} bytes: {:02X?}", buf.len(), &buf[..std::cmp::min(buf.len(), 20)]);
        Ok(buf)
    }

    /// Decode packets into handshake messages
    pub fn decode_handshake(data: &[u8]) -> io::Result<(PacketHeader, HandshakeMessage)> {
        tracing::info!("[SL_CODEC] 🔍 Decoding {} bytes: {:02X?}", data.len(), &data[..std::cmp::min(data.len(), 20)]);

        if data.len() < 7 {
            tracing::warn!("[SL_CODEC] ❌ Packet too short: {} bytes", data.len());
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Packet too short for message ID"));
        }

        let header = PacketHeader {
            sequence_id: u32::from_be_bytes(data[1..5].try_into().unwrap_or_default()),
            flags: data[0],
        };

        let mut frequency = 0;
        while frequency < 3 && data.get(6 + frequency) == Some(&0xFF) {
            frequency += 1;
        }

        let (id, body_offset) = match frequency {
            0 => { // High
                tracing::info!("[SL_CODEC] 🔥 High frequency message detected: 0x{:02X}", data[6]);
                (data[6] as u32, 7)
            },
            1 => { // Medium
                if data.len() < 8 {
                     return Err(io::Error::new(io::ErrorKind::InvalidData, "Packet too short for Medium frequency message ID"));
                }
                tracing::info!("[SL_CODEC] 🔄 Medium frequency message detected: 0xFF{:02X}", data[7]);
                (data[7] as u32, 8)
            },
            2 => { // Low
                if data.len() < 10 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Packet too short for Low frequency message ID"));
                }
                let id = u16::from_be_bytes([data[8], data[9]]);
                tracing::info!("[SL_CODEC] 🌊 Low frequency message detected: 0xFFFF{:04X}", id);
                (id as u32, 10)
            },
            _ => { // Fixed or other
                 if data.len() < 10 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Packet too short for Fixed frequency message ID"));
                }
                let id = u32::from_be_bytes(data[6..10].try_into().unwrap());
                tracing::info!("[SL_CODEC] ⚓ Fixed frequency message detected: 0x{:08X}", id);
                (id, 10)
            }
        };

        match (frequency, id) {
             // High Frequency
            (0, 0) => { // RegionHandshake
                tracing::info!("[SL_CODEC] 📥 High frequency RegionHandshake message (type 0)");
                match super::region_handshake::parse_region_handshake(&data[body_offset..]) {
                    Some(region_data) => Ok((header, HandshakeMessage::RegionHandshake {
                        region_name: region_data.region_name,
                        region_id: region_data.region_id,
                        region_flags: region_data.region_flags,
                        water_height: region_data.water_height,
                        sim_access: region_data.sim_access,
                    })),
                    None => Err(io::Error::new(io::ErrorKind::InvalidData, "Failed to parse RegionHandshake")),
                }
            },
            (0, 11) => { // AgentUpdate
                tracing::info!("[SL_CODEC] 📥 AgentUpdate message (type 11)");
                // This is a placeholder. The actual parsing logic would go here.
                // For now, we'll just acknowledge we received it.
                Ok((header, HandshakeMessage::AgentUpdate{
                    agent_id: Uuid::nil().to_string(),
                    session_id: Uuid::nil().to_string(),
                    position: (0.0, 0.0, 0.0),
                    camera_at: (0.0, 0.0, 0.0),
                    camera_eye: (0.0, 0.0, 0.0),
                    controls: 0,
                }))
            },
            (0, 250) => { // AgentMovementComplete
                tracing::info!("[SL_CODEC] 📥 High frequency AgentMovementComplete message (type 250)");
                if data.len() >= body_offset + 32 {
                    let agent_id = Uuid::from_slice(&data[body_offset..body_offset+16]).map(|u| u.to_string()).unwrap_or_default();
                    let session_id = Uuid::from_slice(&data[body_offset+16..body_offset+32]).map(|u| u.to_string()).unwrap_or_default();
                    Ok((header, HandshakeMessage::AgentMovementComplete { agent_id, session_id }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "AgentMovementComplete packet too short"))
                }
            },

            // Medium Frequency
            (1, 6) => { // ACK list
                tracing::info!("[SL_CODEC] 📥 Medium frequency ACK list message (type 6)");
                 if data.len() >= body_offset + 4 {
                    let acked_seq = u32::from_be_bytes(data[body_offset..body_offset+4].try_into().unwrap());
                    Ok((header, HandshakeMessage::Ack { sequence_id: acked_seq }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "ACK list packet too short"))
                }
            },
            // Low Frequency
            (2, 1) => { // RegionHandshake
                tracing::info!("[SL_CODEC] 📥 Low frequency RegionHandshake message (type 1)");
                match super::region_handshake::parse_region_handshake(&data[body_offset..]) {
                    Some(region_data) => Ok((header, HandshakeMessage::RegionHandshake {
                        region_name: region_data.region_name,
                        region_id: region_data.region_id,
                        region_flags: region_data.region_flags,
                        water_height: region_data.water_height,
                        sim_access: region_data.sim_access,
                    })),
                    None => Err(io::Error::new(io::ErrorKind::InvalidData, "Failed to parse RegionHandshake")),
                }
            },
            (2, 3) => { // UseCircuitCode
                tracing::info!("[SL_CODEC] 📥 UseCircuitCode message");
                if data.len() >= body_offset + 36 {
                    let circuit_code = u32::from_le_bytes(data[body_offset..body_offset+4].try_into().unwrap());
                    let session_id = Uuid::from_slice(&data[body_offset+4..body_offset+20]).map(|u| u.to_string()).unwrap_or_default();
                    let agent_id = Uuid::from_slice(&data[body_offset+20..body_offset+36]).map(|u| u.to_string()).unwrap_or_default();
                    Ok((header, HandshakeMessage::UseCircuitCode { agent_id, session_id, circuit_code }))
                } else {
                     Err(io::Error::new(io::ErrorKind::InvalidData, "UseCircuitCode packet too short"))
                }
            },
            (2, 150) => { // UseCircuitCodeReply
                 tracing::info!("[SL_CODEC] 📥 UseCircuitCodeReply message");
                 if data.len() >= body_offset + 1 {
                     let success = data[body_offset] != 0;
                     Ok((header, HandshakeMessage::UseCircuitCodeReply(success)))
                 } else {
                     Err(io::Error::new(io::ErrorKind::InvalidData, "UseCircuitCodeReply packet too short"))
                 }
            },
             (2, 387) => { // AgentDataUpdate
                tracing::info!("[SL_CODEC] 📥 AgentDataUpdate message (type 387)");
                if data.len() >= body_offset + 16 {
                    let agent_id = Uuid::from_slice(&data[body_offset..body_offset+16]).map(|u| u.to_string()).unwrap_or_default();
                    Ok((header, HandshakeMessage::AgentDataUpdate { agent_id }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "AgentDataUpdate packet too short"))
                }
            },

             (2, 322) => { // OnlineNotification
                tracing::info!("[SL_CODEC] 📥 OnlineNotification message (type 322)");
                if data.len() >= body_offset + 1 {
                    let agent_block_count = data[body_offset];
                    let mut agent_ids = Vec::new();
                    let mut offset = body_offset + 1;

                    for _ in 0..agent_block_count {
                        if offset + 16 <= data.len() {
                            if let Ok(uuid) = Uuid::from_slice(&data[offset..offset+16]) {
                                agent_ids.push(uuid.to_string());
                            }
                            offset += 16;
                        }
                    }
                    Ok((header, HandshakeMessage::OnlineNotification { agent_ids }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "OnlineNotification packet too short"))
                }
            },

             (2, 138) => { // HealthMessage
                tracing::info!("[SL_CODEC] 📥 HealthMessage (type 138)");
                Ok((header, HandshakeMessage::HealthMessage))
            },

             (2, 250) => { // AgentMovementComplete
                tracing::info!("[SL_CODEC] 📥 Low frequency AgentMovementComplete message (type 250)");
                if data.len() >= body_offset + 32 {
                    let agent_id = Uuid::from_slice(&data[body_offset..body_offset+16]).map(|u| u.to_string()).unwrap_or_default();
                    let session_id = Uuid::from_slice(&data[body_offset+16..body_offset+32]).map(|u| u.to_string()).unwrap_or_default();
                    Ok((header, HandshakeMessage::AgentMovementComplete { agent_id, session_id }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "AgentMovementComplete packet too short"))
                }
            },

             (2, 249) => { // AgentMovementComplete
                tracing::info!("[SL_CODEC] 📥 Low frequency AgentMovementComplete message (type 249)");
                if data.len() >= body_offset + 32 {
                    let agent_id = Uuid::from_slice(&data[body_offset..body_offset+16]).map(|u| u.to_string()).unwrap_or_default();
                    let session_id = Uuid::from_slice(&data[body_offset+16..body_offset+32]).map(|u| u.to_string()).unwrap_or_default();
                    Ok((header, HandshakeMessage::AgentMovementComplete { agent_id, session_id }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "AgentMovementComplete packet too short"))
                }
            },

            // Fixed Frequency
            (3, 0xFFFFFFFB) => { // PacketAck
                tracing::info!("[SL_CODEC] 📥 PacketAck message");
                if data.len() >= body_offset + 5 {
                    let _num_packets = data[body_offset];
                    let acked_seq = u32::from_be_bytes(data[body_offset+1..body_offset+5].try_into().unwrap());
                    Ok((header, HandshakeMessage::Ack { sequence_id: acked_seq }))
                } else {
                    Err(io::Error::new(io::ErrorKind::InvalidData, "PacketAck packet too short"))
                }
            },

            _ => {
                tracing::warn!("[SL_CODEC] ❌ Unsupported or unknown message type. Freq: {}, ID: 0x{:X}", frequency, id);
                Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported or unknown message type"))
            }
        }
    }

    /// Test function to demonstrate debug messages
    pub fn test_debug_messages() {
        tracing::info!("[SL_CODEC] 🧪 Testing debug message system...");
        
        let header = PacketHeader {
            sequence_id: 12345,
            flags: 0x40,
        };
        
        let message = HandshakeMessage::UseCircuitCode {
            agent_id: "84c4017d-f3a1-4565-837d-23a67ad0ebd7".to_string(),
            session_id: "12345678-1234-1234-1234-123456789012".to_string(),
            circuit_code: 98765,
        };
        
        tracing::info!("[SL_CODEC] 🧪 Testing encoding...");
        match Self::encode_handshake(&header, &message) {
            Ok(encoded) => {
                tracing::info!("[SL_CODEC] 🧪 Encoding successful! {} bytes", encoded.len());
                
                tracing::info!("[SL_CODEC] 🧪 Testing decoding...");
                match Self::decode_handshake(&encoded) {
                    Ok((decoded_header, decoded_message)) => {
                        tracing::info!("[SL_CODEC] 🧪 Decoding successful!");
                        tracing::info!("[SL_CODEC] 🧪 Header: seq={}, flags=0x{:02X}", decoded_header.sequence_id, decoded_header.flags);
                    },
                    Err(e) => tracing::error!("[SL_CODEC] 🧪 Decoding failed: {}", e),
                }
            },
            Err(e) => tracing::error!("[SL_CODEC] 🧪 Encoding failed: {}", e),
        }
        
        tracing::info!("[SL_CODEC] 🧪 Debug message test complete!");
    }
}