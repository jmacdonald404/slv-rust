use uuid::Uuid;
use bytes::{BytesMut, BufMut, Buf};

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

/// Build a UseCircuitCode UDP packet according to the message_template.msg protocol:
/// [flags: u8][sequence_number: u32 LE][message_type_id: u32 LE][circuit_code: u32 LE][session_id: [u8;16]][agent_id: [u8;16]]
pub fn build_usecircuitcode_packet_lludp(
    circuit_code: u32,
    session_id: Uuid,
    agent_id: Uuid,
    sequence_number: u32,
) -> BytesMut {
    let mut buf = BytesMut::with_capacity(1 + 4 + 4 + 4 + 16 + 16);
    buf.put_u8(0x00); // Flags
    buf.put_u32_le(sequence_number); // Sequence number (little-endian)
    buf.put_u32_le(3); // Message type ID for UseCircuitCode (u32, little-endian)
    buf.put_u32_le(circuit_code); // Circuit code (little-endian)
    buf.put_slice(session_id.as_bytes());
    buf.put_slice(agent_id.as_bytes());
    buf
}

/*
Example usage for sending UseCircuitCode:
let payload = build_usecircuitcode_payload(circuit_code, session_id, agent_id);
let packet = LluPacket::build_outgoing(0x0009, LluPacketFlags::RELIABLE, Some(seq), &payload);
udp.send_to(&packet, &sim_addr).await;
*/ 