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
                buf.extend_from_slice(&sequence_id.to_le_bytes());
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

        let id_byte1 = data[6];
        tracing::info!("[SL_CODEC] 🎯 Header parsed: seq={}, flags=0x{:02X}, first_msg_byte=0x{:02X}", header.sequence_id, header.flags, id_byte1);
        
        // High Frequency Messages (single byte)
        if id_byte1 < 0xFF {
            tracing::info!("[SL_CODEC] 🔥 High frequency message detected: 0x{:02X}", id_byte1);
            match id_byte1 {
                0 => { // RegionHandshake - High frequency message type 0
                    tracing::info!("[SL_CODEC] 📥 High frequency RegionHandshake message (type 0)");
                    // Parse RegionHandshake data using the manual parser
                    if data.len() > 7 {
                        match super::region_handshake::parse_region_handshake(&data[7..]) {
                            Some(region_data) => {
                                tracing::info!("[SL_CODEC] ✅ RegionHandshake decoded: region_id={}, flags={}, water_height={}", 
                                    region_data.region_id, region_data.region_flags, region_data.water_height);
                                return Ok((header, HandshakeMessage::RegionHandshake {
                                    region_name: region_data.region_name,
                                    region_id: region_data.region_id,
                                    region_flags: region_data.region_flags,
                                    water_height: region_data.water_height,
                                    sim_access: region_data.sim_access,
                                }));
                            },
                            None => {
                                tracing::warn!("[SL_CODEC] ❌ Failed to parse RegionHandshake data");
                                return Ok((header, HandshakeMessage::RegionHandshake {
                                    region_name: "Parse_Error".to_string(),
                                    region_id: Uuid::nil(),
                                    region_flags: 0,
                                    water_height: 0.0,
                                    sim_access: 0,
                                }));
                            }
                        }
                    }
                },
                1 => { // StartPingCheck (High frequency message 1) 
                    tracing::info!("[SL_CODEC] 📥 StartPingCheck message - not implemented");
                    // StartPingCheck messages are not part of our handshake flow
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "StartPingCheck not supported in handshake"));
                },
                5 => { // RegionHandshake (legacy support)
                    tracing::info!("[SL_CODEC] 📥 RegionHandshake message (type 5 - legacy)");
                    // TODO: Parse actual RegionHandshake data
                    tracing::info!("[SL_CODEC] ✅ RegionHandshake decoded (placeholder)");
                    return Ok((header, HandshakeMessage::RegionHandshake {
                        region_name: "Unknown".to_string(),
                        region_id: Uuid::nil(),
                        region_flags: 0,
                        water_height: 0.0,
                        sim_access: 0,
                    }));
                },
                250 => { // AgentMovementComplete - High frequency message type 250 (0xFA)
                    tracing::info!("[SL_CODEC] 📥 High frequency AgentMovementComplete message (type 250)");
                    if data.len() >= 39 { // 7 (header) + 16 (agent_id) + 16 (session_id) = 39 minimum
                        let agent_id = Uuid::from_slice(&data[7..23]).map(|u| u.to_string()).unwrap_or_default();
                        let session_id = Uuid::from_slice(&data[23..39]).map(|u| u.to_string()).unwrap_or_default();
                        tracing::info!("[SL_CODEC] ✅ AgentMovementComplete decoded: agent_id={}", agent_id);
                        return Ok((header, HandshakeMessage::AgentMovementComplete { agent_id, session_id }));
                    } else {
                        tracing::warn!("[SL_CODEC] ❌ AgentMovementComplete packet too short: {} bytes", data.len());
                    }
                },
                _ => {}
            }
        }

        // Medium/Low Frequency Messages
        if data.len() >= 10 {
            let full_id = &data[6..10];
            tracing::info!("[SL_CODEC] 🌊 Medium/Low frequency message: {:02X?}", full_id);
            match full_id {
                [0xFF, 0xFF, 0x00, 0x03] => { // UseCircuitCode (3)
                    tracing::info!("[SL_CODEC] 📥 UseCircuitCode message");
                    if data.len() >= 42 {
                        let circuit_code = u32::from_le_bytes(data[10..14].try_into().unwrap_or_default());
                        let session_id = Uuid::from_slice(&data[14..30]).map(|u| u.to_string()).unwrap_or_default();
                        let agent_id = Uuid::from_slice(&data[30..46]).map(|u| u.to_string()).unwrap_or_default();
                        tracing::info!("[SL_CODEC] ✅ UseCircuitCode decoded: circuit_code={}", circuit_code);
                        return Ok((header, HandshakeMessage::UseCircuitCode { agent_id, session_id, circuit_code }));
                    }
                },
                [0xFF, 0xFF, 0xFF, 0xFB] => { // PacketAck (Fixed message 0xFFFFFFFB)
                    tracing::info!("[SL_CODEC] 📥 PacketAck message");
                    if data.len() >= 15 { // 10 (header) + 1 (var block count) + 4 (U32 packet ID)
                        let _num_packets = data[10]; // Variable block count
                        let acked_seq = u32::from_be_bytes(data[11..15].try_into().unwrap_or_default());
                        tracing::info!("[SL_CODEC] ✅ PacketAck decoded: acked_seq={}", acked_seq);
                        return Ok((header, HandshakeMessage::Ack { sequence_id: acked_seq }));
                    } else {
                        tracing::warn!("[SL_CODEC] ❌ PacketAck packet too short: {} bytes", data.len());
                    }
                },
                [0xFF, 0xFF, 0x00, 0x96] => { // UseCircuitCodeReply (150)
                    tracing::info!("[SL_CODEC] 📥 UseCircuitCodeReply message");
                    if data.len() >= 11 {
                        let success = data[10] != 0;
                        tracing::info!("[SL_CODEC] ✅ UseCircuitCodeReply decoded: success={}", success);
                        return Ok((header, HandshakeMessage::UseCircuitCodeReply(success)));
                    }
                },
                [0xFF, 0xFF, 0x00, 0x01] => { // RegionHandshake (Low frequency message 1)
                    tracing::info!("[SL_CODEC] 📥 Low frequency RegionHandshake message (type 1)");
                    // Parse RegionHandshake data using the manual parser
                    if data.len() > 10 {
                        match super::region_handshake::parse_region_handshake(&data[10..]) {
                            Some(region_data) => {
                                tracing::info!("[SL_CODEC] ✅ RegionHandshake decoded: region_id={}, flags={}, water_height={}", 
                                    region_data.region_id, region_data.region_flags, region_data.water_height);
                                return Ok((header, HandshakeMessage::RegionHandshake {
                                    region_name: region_data.region_name,
                                    region_id: region_data.region_id,
                                    region_flags: region_data.region_flags,
                                    water_height: region_data.water_height,
                                    sim_access: region_data.sim_access,
                                }));
                            },
                            None => {
                                tracing::warn!("[SL_CODEC] ❌ Failed to parse RegionHandshake data");
                                return Ok((header, HandshakeMessage::RegionHandshake {
                                    region_name: "Parse_Error".to_string(),
                                    region_id: Uuid::nil(),
                                    region_flags: 0,
                                    water_height: 0.0,
                                    sim_access: 0,
                                }));
                            }
                        }
                    } else {
                        tracing::warn!("[SL_CODEC] ❌ RegionHandshake packet too short: {} bytes", data.len());
                        return Ok((header, HandshakeMessage::RegionHandshake {
                            region_name: "Too_Short".to_string(),
                            region_id: Uuid::nil(),
                            region_flags: 0,
                            water_height: 0.0,
                            sim_access: 0,
                        }));
                    }
                },
                [0xFF, 0xFF, 0x00, 0xF9] => { // AgentMovementComplete
                    if data.len() >= 42 {
                        let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
                        let session_id = Uuid::from_slice(&data[26..42]).map(|u| u.to_string()).unwrap_or_default();
                        return Ok((header, HandshakeMessage::AgentMovementComplete { agent_id, session_id }));
                    }
                },
                [0xFF, 0xFF, 0x00, 0xFA] => { // AgentMovementComplete (Low frequency message 250)
                    tracing::info!("[SL_CODEC] 📥 Low frequency AgentMovementComplete message (type 250)");
                    if data.len() >= 42 { // 10 (header+msg_id) + 16 (agent_id) + 16 (session_id) = 42 minimum
                        let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
                        let session_id = Uuid::from_slice(&data[26..42]).map(|u| u.to_string()).unwrap_or_default();
                        tracing::info!("[SL_CODEC] ✅ AgentMovementComplete decoded: agent_id={}", agent_id);
                        return Ok((header, HandshakeMessage::AgentMovementComplete { agent_id, session_id }));
                    } else {
                        tracing::warn!("[SL_CODEC] ❌ AgentMovementComplete packet too short: {} bytes", data.len());
                    }
                },
                [0xFF, 0xFF, 0x01, 0x83] => { // AgentDataUpdate
                    if data.len() >= 26 {
                        let agent_id = Uuid::from_slice(&data[10..26]).map(|u| u.to_string()).unwrap_or_default();
                        return Ok((header, HandshakeMessage::AgentDataUpdate { agent_id }));
                    }
                },
                [0xFF, 0xFF, 0x00, 0x8A] => { // HealthMessage
                    return Ok((header, HandshakeMessage::HealthMessage));
                },
                _ => {}
            }
        }

        // Medium frequency messages (0xFF + single byte)
        if data.len() >= 8 && data[6] == 0xFF && data[7] != 0xFF {
            tracing::info!("[SL_CODEC] 🔄 Medium frequency message: 0xFF{:02X}", data[7]);
            match data[7] {
                0x06 => { // ACK list message
                    tracing::info!("[SL_CODEC] 📥 Medium frequency ACK list message");
                    if data.len() >= 12 {
                        let acked_seq = u32::from_be_bytes(data[8..12].try_into().unwrap_or_default());
                        tracing::info!("[SL_CODEC] ✅ Medium ACK decoded: acked_seq={}", acked_seq);
                        return Ok((header, HandshakeMessage::Ack { sequence_id: acked_seq }));
                    }
                },
                _ => {
                    tracing::warn!("[SL_CODEC] ❓ Unknown medium frequency message: 0xFF{:02X}", data[7]);
                }
            }
        }

        tracing::warn!("[SL_CODEC] ❌ Unsupported or unknown message type");
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported or unknown message type"))
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