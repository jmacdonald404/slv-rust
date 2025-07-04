use bincode::{encode_to_vec, decode_from_slice};
use crate::networking::protocol::messages::{PacketHeader, Message};
use std::io::{self, ErrorKind};

pub struct MessageCodec;

impl MessageCodec {
    pub fn encode(header: &PacketHeader, message: &Message) -> io::Result<Vec<u8>> {
        let mut encoded = encode_to_vec(header, bincode::config::standard())
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        encoded.extend_from_slice(&encode_to_vec(message, bincode::config::standard())
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?);
        Ok(encoded)
    }

    pub fn decode(data: &[u8]) -> io::Result<(PacketHeader, Message)> {
        let (header, header_len): (PacketHeader, usize) = decode_from_slice(data, bincode::config::standard())
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let (message, _): (Message, usize) = decode_from_slice(&data[header_len..], bincode::config::standard())
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        Ok((header, message))
    }
}
