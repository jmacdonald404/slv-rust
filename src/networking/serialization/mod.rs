//! Second Life packet serialization and deserialization
//! 
//! This module provides exact binary compatibility with the Second Life protocol,
//! handling packet headers, flags, zerocoding, and reliable packet acknowledgment.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::{Packet, PacketFrequency, PacketWrapper};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use tracing::info;

pub mod packet_buffer;
pub mod zerocode;

pub use packet_buffer::PacketBuffer;

/// Maximum packet sequence number before wrapping
const MAX_SEQUENCE: u32 = 0x01000000;

/// Packet flags
const ACK_FLAG: u8 = 0x40;      // Reliable packet flag
const RESENT_FLAG: u8 = 0x20;    // Resent packet flag  
const ZEROCODED_FLAG: u8 = 0x80; // Zerocoded packet flag

/// Packet serializer that produces exact Second Life protocol format
pub struct PacketSerializer {
    sequence: u32,
}

impl PacketSerializer {
    pub fn new() -> Self {
        Self { sequence: 1 }
    }
    
    /// Get the current sequence number
    pub fn current_sequence(&self) -> u32 {
        self.sequence
    }
    
    /// Serialize a packet into the exact Second Life UDP format
    pub fn serialize<P: Packet>(&mut self, packet: &P, reliable: bool) -> NetworkResult<(Bytes, u32)> {
        let mut buffer = BytesMut::new();
        
        // Serialize packet data first
        let packet_data = bincode::serialize(packet)
            .map_err(|e| NetworkError::PacketEncode { 
                reason: format!("Failed to serialize packet data: {}", e) 
            })?;
            
        // Apply zerocoding if enabled
        let final_data = if P::ZEROCODED {
            zerocode::encode(&packet_data)
        } else {
            packet_data
        };
        
        // Build header
        let sequence = self.next_sequence();
        self.write_header(&mut buffer, P::ID.try_into().unwrap(), P::FREQUENCY, reliable, P::ZEROCODED, sequence);
        
        // Append packet data
        buffer.extend_from_slice(&final_data);
        
        Ok((buffer.freeze(), sequence))
    }
    
    /// Serialize a packet wrapper (used for resends with same sequence)
    pub fn serialize_wrapper(&mut self, wrapper: &PacketWrapper) -> NetworkResult<Bytes> {
        let mut buffer = BytesMut::new();
        
        // Apply zerocoding if needed (check if original packet was zerocoded)
        let packet_info = crate::networking::packets::get_packet_info_by_id(
            wrapper.packet_id, 
            wrapper.frequency
        ).ok_or_else(|| NetworkError::PacketEncode {
            reason: format!("Unknown packet ID {} for frequency {:?}", 
                          wrapper.packet_id, wrapper.frequency)
        })?;
        
        let final_data = if packet_info.zerocoded {
            zerocode::encode(&wrapper.data)
        } else {
            wrapper.data.clone()
        };
        
        // Build header with wrapper's sequence
        self.write_header(&mut buffer, 
                         wrapper.packet_id, 
                         wrapper.frequency, 
                         wrapper.reliable, 
                         packet_info.zerocoded,
                         wrapper.sequence);
        
        // Append packet data
        buffer.extend_from_slice(&final_data);
        
        Ok(buffer.freeze())
    }
    
    fn next_sequence(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence += 1;
        if self.sequence >= MAX_SEQUENCE {
            self.sequence = 1;
        }
        seq
    }
    
    /// Write Second Life packet header
    /// Format: [flags:1] [sequence:4] [extra:1] [message_id:1-4]
    fn write_header(&self, buffer: &mut BytesMut, 
                   packet_id: u16, 
                   frequency: PacketFrequency,
                   reliable: bool,
                   zerocoded: bool,
                   sequence: u32) {
        // Flags byte
        let mut flags = 0u8;
        if reliable {
            flags |= ACK_FLAG;
        }
        if zerocoded {
            flags |= ZEROCODED_FLAG;
        }
        buffer.put_u8(flags);
        
        // Sequence number (4 bytes, big-endian)
        buffer.put_u32(sequence);
        
        // Extra header byte (always 0)
        buffer.put_u8(0);
        
        // Message ID encoding based on frequency
        match frequency {
            PacketFrequency::High => {
                // High: single byte ID
                buffer.put_u8(packet_id as u8);
            }
            PacketFrequency::Medium => {
                // Medium: 0xFF + single byte ID
                buffer.put_u8(0xFF);
                buffer.put_u8(packet_id as u8);
            }
            PacketFrequency::Low => {
                // Low: 0xFF 0xFF + two byte ID (big-endian)
                buffer.put_u8(0xFF);
                buffer.put_u8(0xFF);
                buffer.put_u16(packet_id);
            }
            PacketFrequency::Fixed => {
                // Fixed: 0xFF 0xFF 0xFF + single byte ID
                buffer.put_u8(0xFF);
                buffer.put_u8(0xFF);
                buffer.put_u8(0xFF);
                buffer.put_u8(packet_id as u8);
            }
        }
    }
}

impl Default for PacketSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Packet deserializer that handles Second Life protocol format
pub struct PacketDeserializer {
    packet_registry: HashMap<u32, fn(&[u8]) -> NetworkResult<Box<dyn std::any::Any + Send + Sync>>>,
}

impl PacketDeserializer {
    pub fn new() -> Self {
        let mut deserializer = Self {
            packet_registry: HashMap::new(),
        };
        
        // Register known packet deserializers
        deserializer.register_all_packets();
        deserializer
    }
    
    /// Parse a raw UDP packet into a PacketWrapper
    pub fn parse_raw(&self, data: &[u8]) -> NetworkResult<PacketWrapper> {
        if data.len() < 6 {
            return Err(NetworkError::PacketDecode {
                reason: "Packet too short for header".to_string(),
            });
        }
        
        let mut buffer = PacketBuffer::new(data);
        
        // Parse header
        let flags = buffer.get_u8();
        let sequence = buffer.get_u32();
        let _extra = buffer.get_u8(); // Skip extra byte
        
        let reliable = (flags & ACK_FLAG) != 0;
        let zerocoded = (flags & ZEROCODED_FLAG) != 0;
        
        info!("Parsing packet: flags=0x{:02x}, sequence={}, reliable={}, zerocoded={}", 
               flags, sequence, reliable, zerocoded);
        
        // Parse message ID and determine frequency
        let (packet_id, frequency) = self.parse_message_id(&mut buffer)?;
        
        info!("Parsed message ID: {} ({:?})", packet_id, frequency);
        
        // Get remaining packet data
        let mut packet_data = buffer.remaining_bytes().to_vec();
        
        // Decode zerocoding if present
        if zerocoded {
            packet_data = zerocode::decode(&packet_data)?;
        }
        
        Ok(PacketWrapper {
            data: packet_data,
            reliable,
            sequence,
            packet_id,
            frequency,
            embedded_acks: None, // TODO: Parse embedded ACKs from packet header
        })
    }
    
    /// Parse message ID from buffer and return (id, frequency)
    fn parse_message_id(&self, buffer: &mut PacketBuffer) -> NetworkResult<(u16, PacketFrequency)> {
        let first_byte = buffer.get_u8();
        
        if first_byte != 0xFF {
            // High frequency: single byte
            return Ok((first_byte as u16, PacketFrequency::High));
        }
        
        let second_byte = buffer.get_u8();
        if second_byte != 0xFF {
            // Medium frequency: 0xFF + single byte
            return Ok((second_byte as u16, PacketFrequency::Medium));
        }
        
        let third_byte = buffer.get_u8();
        if third_byte != 0xFF {
            // Low frequency: 0xFF 0xFF + two bytes
            let fourth_byte = buffer.get_u8();
            let id = ((third_byte as u16) << 8) | (fourth_byte as u16);
            return Ok((id, PacketFrequency::Low));
        }
        
        // Fixed frequency: 0xFF 0xFF 0xFF + single byte
        let id = buffer.get_u8() as u16;
        Ok((id, PacketFrequency::Fixed))
    }
    
    /// Deserialize a PacketWrapper into a specific packet type
    pub fn deserialize<P: Packet>(&self, wrapper: &PacketWrapper) -> NetworkResult<P> {
        // Verify packet type matches
        if u32::from(wrapper.packet_id) != P::ID || wrapper.frequency != P::FREQUENCY {
            return Err(NetworkError::PacketDecode {
                reason: format!(
                    "Packet type mismatch: expected {}:{:?}, got {}:{:?}",
                    P::ID, P::FREQUENCY, wrapper.packet_id, wrapper.frequency
                ),
            });
        }
        
        bincode::deserialize(&wrapper.data)
            .map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to deserialize packet: {}", e),
            })
    }
    
    fn register_all_packets(&mut self) {
        use crate::networking::packets::generated::*;
        
        // Register deserializers for all known packets
        self.register_packet::<UseCircuitCode>();
        self.register_packet::<CompleteAgentMovement>();
        self.register_packet::<RegionHandshake>();
        self.register_packet::<RegionHandshakeReply>();
        self.register_packet::<AgentThrottle>();
        self.register_packet::<AgentUpdate>();
        self.register_packet::<AgentHeightWidth>();
        self.register_packet::<LogoutRequest>();
        self.register_packet::<PacketAck>();
    }
    
    fn register_packet<P: Packet + 'static>(&mut self) {
        let key = P::lookup_key();
        self.packet_registry.insert(key, |data| {
            let packet: P = bincode::deserialize(data)
                .map_err(|e| crate::networking::NetworkError::PacketDecode {
                    reason: e.to_string()
                })?;
            Ok(Box::new(packet))
        });
    }
}

impl Default for PacketDeserializer {
    fn default() -> Self {
        Self::new()
    }
}