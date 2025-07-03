use bincode::{serialize, deserialize};
use crate::networking::protocol::messages::{PacketHeader, Message};
use std::io::{self, ErrorKind};

pub struct MessageCodec;

impl MessageCodec {
    pub fn encode(header: &PacketHeader, message: &Message) -> io::Result<Vec<u8>> {
        let mut encoded = serialize(header)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        encoded.extend_from_slice(&serialize(message)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?);
        Ok(encoded)
    }

    pub fn decode(data: &[u8]) -> io::Result<(PacketHeader, Message)> {
        let header: PacketHeader = deserialize(data)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let header_len = serialize(&header).unwrap().len(); // This is a bit hacky, need a better way to get header size
        let message: Message = deserialize(&data[header_len..])
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        Ok((header, message))
    }
}
