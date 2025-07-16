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
    _zerocode_enabled: bool, // ignored for UseCircuitCode
) -> Vec<u8> {
    let mut buf = Vec::new();
    let flags: u8 = 0x40; // RELIABLE, unencoded
    buf.push(flags); // 1 byte flags
    buf.extend_from_slice(&1u32.to_be_bytes()); // 4 bytes packet id, always 1
    buf.push(0x00); // 1 byte offset, always 0
    buf.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x03]); // 4 bytes message number (UseCircuitCode)
    buf.extend_from_slice(&circuit_code.to_le_bytes()); // 4 bytes circuit code
    buf.extend_from_slice(session_id.as_bytes()); // 16 bytes session id
    buf.extend_from_slice(agent_id.as_bytes()); // 16 bytes agent id
    buf
}

/// Stub for a generic LLUDP message builder (to be implemented for other message types)
pub fn build_lludp_packet_stub(
    _frequency: LLUDPFrequency,
    _msg_id: u8,
    _packet_id: u32,
    _body: &[u8],
) -> Vec<u8> {
    // TODO: Implement frequency encoding and message body packing for arbitrary messages
    unimplemented!("Generic LLUDP message builder not yet implemented");
} 