//! LLUDP packet builder for Second Life/SL-compatible protocols.
//!
//! Provides helpers for constructing LLUDP packets with correct frequency encoding.

use bytes::{BytesMut, BufMut};
use uuid::Uuid;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct LluPacketFlags: u8 {
        const RELIABLE = 0x01;
        const ZEROCODED = 0x80;
        // Add more flags as needed
    }
}

#[derive(Debug, Clone)]
pub struct LluPacket {
    pub message_id: u16,
    pub flags: LluPacketFlags,
    pub sequence: Option<u32>, // Some if reliable, None if not
    pub payload: BytesMut,
}

impl LluPacket {
    /// Build an outgoing LLUDP packet
    pub fn build_outgoing(message_id: u16, flags: LluPacketFlags, sequence: Option<u32>, payload: &[u8]) -> BytesMut {
        let mut buf = BytesMut::with_capacity(2 + 1 + if flags.contains(LluPacketFlags::RELIABLE) { 4 } else { 0 } + payload.len());
        buf.put_u16_le(message_id);
        buf.put_u8(flags.bits());
        if let Some(seq) = sequence {
            buf.put_u32_le(seq);
        }
        buf.put_slice(payload);
        buf
    }

    /// Parse an incoming LLUDP packet
    pub fn parse_incoming(mut data: &[u8]) -> Option<LluPacket> {
        if data.len() < 3 { return None; }
        let message_id = u16::from_le_bytes([data[0], data[1]]);
        let flags = LluPacketFlags::from_bits_truncate(data[2]);
        let mut offset = 3;
        let sequence = if flags.contains(LluPacketFlags::RELIABLE) {
            if data.len() < 7 { return None; }
            let seq = u32::from_le_bytes([data[3], data[4], data[5], data[6]]);
            offset += 4;
            Some(seq)
        } else {
            None
        };
        let payload = BytesMut::from(&data[offset..]);
        Some(LluPacket { message_id, flags, sequence, payload })
    }
}

/// Frequency encoding for LLUDP message numbers.
#[derive(Debug, Clone, Copy)]
pub enum LLUDPFrequency {
    Low,
    Medium,
    High,
    Fixed,
}

/// Zerocode an LLUDP payload (Second Life protocol zero-compression)
pub fn zerocode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == 0 {
            // Count consecutive zeros
            let mut count = 1u8;
            while i + (count as usize) < input.len() && input[i + count as usize] == 0 && count < 255 {
                count += 1;
            }
            out.push(0);
            out.push(count);
            i += count as usize;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    out
}

/// Decode a zerocoded LLUDP payload (for testing)
pub fn zerodecode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == 0 {
            if i + 1 >= input.len() {
                break; // Malformed input
            }
            let count = input[i + 1];
            for _ in 0..count {
                out.push(0);
            }
            i += 2;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_zerocode_roundtrip() {
        let cases = vec![
            vec![],
            vec![1, 2, 3],
            vec![0, 0, 0, 0],
            vec![1, 0, 2, 0, 0, 3, 0],
            vec![0],
            vec![0, 1, 0, 2, 0, 3, 0],
            vec![1, 2, 0, 0, 0, 3, 4, 0, 0, 5],
            vec![0; 255],
            vec![0; 256],
            vec![1, 0, 0, 0, 0, 0, 2],
        ];
        for case in cases {
            let encoded = zerocode(&case);
            let decoded = zerodecode(&encoded);
            assert_eq!(case, decoded, "Failed roundtrip for input: {:?}", case);
        }
    }
}

/// Build a UseCircuitCode LLUDP packet (Low frequency, ID 3) as RELIABLE and unencoded (flags = 0x40, no zerocoding)
/// This always uses packet_id = 1, offset = 0, and message number = 0xFF,0xFF,0x00,0x03 (matching the official viewer).
pub fn build_use_circuit_code_packet(
    circuit_code: u32,
    session_id: Uuid,
    agent_id: Uuid,
    packet_id: u32,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let flags: u8 = 0x40; // RELIABLE, unencoded
    buf.push(flags); // 1 byte flags
    buf.extend_from_slice(&packet_id.to_be_bytes()); // 4 bytes packet id, big-endian for proxy compatibility
    buf.push(0x00); // 1 byte offset, always 0
    buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x03]); // 4 bytes message number (UseCircuitCode)
    buf.extend_from_slice(&circuit_code.to_le_bytes()); // 4 bytes circuit code
    buf.extend_from_slice(session_id.as_bytes()); // 16 bytes session id
    buf.extend_from_slice(agent_id.as_bytes()); // 16 bytes agent id
    // Debug print for field breakdown
    if cfg!(debug_assertions) {
        let id = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        println!("[DEBUG] UseCircuitCode Packet:");
        println!("  flags:        {:02X}", buf[0]);
        println!("  packet_id:    {} (le: {:02X?})", id, &buf[1..5]);
        println!("  offset:       {:02X}", buf[5]);
        println!("  msg_num:      {:02X?}", &buf[6..10]);
        println!("  circuit_code: {:02X?}", &buf[10..14]);
        println!("  session_id:   {:02X?}", &buf[14..30]);
        println!("  agent_id:     {:02X?}", &buf[30..46]);
        println!("  full:         {:02X?}", buf);
    }
    buf
}

/// Build a generic LLUDP packet.
pub fn build_lludp_packet(
    message_id: u16,
    frequency: LLUDPFrequency,
    packet_id: u32,
    reliable: bool,
    zerocoded: bool,
    body: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::new();

    let mut flags: u8 = 0x00;
    if reliable {
        flags |= LluPacketFlags::RELIABLE.bits();
    }
    if zerocoded {
        flags |= LluPacketFlags::ZEROCODED.bits();
    }

    buf.push(flags); // 1 byte flags
    buf.extend_from_slice(&packet_id.to_le_bytes()); // 4 bytes packet id
    buf.push(0x00); // 1 byte offset, always 0 for now

    // Message number based on frequency
    match frequency {
        LLUDPFrequency::High => {
            buf.extend_from_slice(&[0x00, 0x00, 0x00]);
        }
        LLUDPFrequency::Medium => {
            buf.extend_from_slice(&[0xFF, 0x00, 0x00]);
        }
        LLUDPFrequency::Low => {
            buf.extend_from_slice(&[0xFF, 0xFF, 0x00]);
        }
        LLUDPFrequency::Fixed => {
            buf.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
        }
    }
    buf.push(message_id as u8); // Last byte of message number

    let mut final_body = body.to_vec();
    if zerocoded {
        final_body = zerocode(body);
    }

    buf.extend_from_slice(&final_body);
    buf
}

/// Build a CompleteAgentMovement LLUDP packet (Low frequency, ID 249) as RELIABLE and unencoded (flags = 0x40, no zerocoding)
/// Per SL protocol and Hippolyzer, only AgentID, SessionID, CircuitCode are included in the AgentData block.
/// No extra fields or padding.
pub fn build_complete_agent_movement_packet(
    agent_id: Uuid,
    session_id: Uuid,
    circuit_code: u32,
    packet_id: u32,
    _position: (f32, f32, f32), // Ignored for protocol compliance
    _look_at: (f32, f32, f32),  // Ignored for protocol compliance
) -> Vec<u8> {
    let mut buf = Vec::new();
    let flags: u8 = 0x40; // RELIABLE, unencoded
    buf.push(flags); // 1 byte flags
    buf.extend_from_slice(&packet_id.to_be_bytes()); // 4 bytes packet id
    buf.push(0x00); // 1 byte offset, always 0
    buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0xF9]); // 4 bytes message number (CompleteAgentMovement is Low 249)
    buf.extend_from_slice(agent_id.as_bytes()); // 16 bytes agent id
    buf.extend_from_slice(session_id.as_bytes()); // 16 bytes session id
    buf.extend_from_slice(&circuit_code.to_le_bytes()); // 4 bytes circuit code
    if cfg!(debug_assertions) {
        let id = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        println!("[DEBUG] CompleteAgentMovement Packet:");
        println!("  flags:        {:02X}", buf[0]);
        println!("  packet_id:    {} (le: {:02X?})", id, &buf[1..5]);
        println!("  offset:       {:02X}", buf[5]);
        println!("  msg_num:      {:02X?}", &buf[6..10]);
        println!("  agent_id:     {:02X?}", &buf[10..26]);
        println!("  session_id:   {:02X?}", &buf[26..42]);
        println!("  circuit_code: {:02X?}", &buf[42..46]);
        println!("  full:         {:02X?}", buf);
    }
    buf
}

/// Build a RegionHandshakeReply LLUDP packet (High frequency, ID 6) as RELIABLE and unencoded
pub fn build_region_handshake_reply_packet(
    agent_id: Uuid,
    session_id: Uuid,
    flags: u32,
    packet_id: u32,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let flags_byte: u8 = 0x40; // RELIABLE, unencoded
    buf.push(flags_byte);
    buf.extend_from_slice(&packet_id.to_be_bytes());
    buf.push(0x00);
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x06]); // message number (High Freq 6)
    buf.extend_from_slice(agent_id.as_bytes());
    buf.extend_from_slice(session_id.as_bytes());
    buf.extend_from_slice(&flags.to_le_bytes());
    if cfg!(debug_assertions) {
        let id = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        println!("[DEBUG] RegionHandshakeReply Packet:");
        println!("  flags:        {:02X}", buf[0]);
        println!("  packet_id:    {} (le: {:02X?})", id, &buf[1..5]);
        println!("  offset:       {:02X}", buf[5]);
        println!("  msg_num:      {:02X?}", &buf[6..10]);
        println!("  agent_id:     {:02X?}", &buf[10..26]);
        println!("  session_id:   {:02X?}", &buf[26..42]);
        println!("  flags:        {:02X?}", &buf[42..46]);
        println!("  full:         {:02X?}", buf);
    }
    buf
}

/// Build an AgentThrottle LLUDP packet (Low frequency, ID 81) as RELIABLE and unencoded
pub fn build_agent_throttle_packet(
    agent_id: Uuid,
    session_id: Uuid,
    circuit_code: u32,
    throttle: [f32; 7],
    packet_id: u32,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let flags: u8 = 0x40;
    buf.push(flags);
    buf.extend_from_slice(&packet_id.to_be_bytes());
    buf.push(0x00);
    buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x51]);
    buf.extend_from_slice(agent_id.as_bytes());
    buf.extend_from_slice(session_id.as_bytes());
    buf.extend_from_slice(&circuit_code.to_be_bytes());
    for v in throttle.iter() {
        buf.extend_from_slice(&v.to_be_bytes());
    }
    if cfg!(debug_assertions) {
        let id = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
        println!("[DEBUG] AgentThrottle Packet:");
        println!("  flags:        {:02X}", buf[0]);
        println!("  packet_id:    {} (be: {:02X?})", id, &buf[1..5]);
        println!("  offset:       {:02X}", buf[5]);
        println!("  msg_num:      {:02X?}", &buf[6..10]);
        println!("  agent_id:     {:02X?}", &buf[10..26]);
        println!("  session_id:   {:02X?}", &buf[26..42]);
        println!("  circuit_code: {:02X?}", &buf[42..46]);
        println!("  throttle:     {:02X?}", &buf[46..74]);
        println!("  full:         {:02X?}", buf);
    }
    buf
}

/// Build an AgentUpdate LLUDP packet (High frequency, ID 4) as UNRELIABLE and unencoded
pub fn build_agent_update_packet(
    agent_id: Uuid,
    session_id: Uuid,
    position: (f32, f32, f32),
    camera_at: (f32, f32, f32),
    camera_eye: (f32, f32, f32),
    controls: u32,
    packet_id: u32,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let flags: u8 = 0x00; // UNRELIABLE, unencoded
    buf.push(flags);
    buf.extend_from_slice(&packet_id.to_be_bytes());
    buf.push(0x00);
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x04]); // message number (high frequency)
    buf.extend_from_slice(agent_id.as_bytes());
    buf.extend_from_slice(session_id.as_bytes());
    for v in [position.0, position.1, position.2, camera_at.0, camera_at.1, camera_at.2, camera_eye.0, camera_eye.1, camera_eye.2].iter() {
        buf.extend_from_slice(&v.to_be_bytes());
    }
    buf.extend_from_slice(&controls.to_be_bytes());
    if cfg!(debug_assertions) {
        let id = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
        println!("[DEBUG] AgentUpdate Packet:");
        println!("  flags:        {:02X}", buf[0]);
        println!("  packet_id:    {} (be: {:02X?})", id, &buf[1..5]);
        println!("  offset:       {:02X}", buf[5]);
        println!("  msg_num:      {:02X?}", &buf[6..10]);
        println!("  agent_id:     {:02X?}", &buf[10..26]);
        println!("  session_id:   {:02X?}", &buf[26..42]);
        println!("  position:     {:02X?}", &buf[42..54]);
        println!("  camera_at:    {:02X?}", &buf[54..66]);
        println!("  camera_eye:   {:02X?}", &buf[66..78]);
        println!("  controls:     {:02X?}", &buf[78..82]);
        println!("  full:         {:02X?}", buf);
    }
    buf
} 