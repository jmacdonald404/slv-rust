use crate::networking::transport::UdpTransport;
use crate::networking::protocol::messages::{PacketHeader, Message};
use crate::networking::protocol::codecs::MessageCodec;
use std::net::SocketAddr;
use std::io;

pub struct Circuit {
    transport: UdpTransport,
    remote_addr: SocketAddr,
}

impl Circuit {
    pub async fn new(bind_addr: &str, remote_addr: SocketAddr) -> std::io::Result<Self> {
        let transport = UdpTransport::new(bind_addr).await?;
        Ok(Self { transport, remote_addr })
    }

    pub async fn send_message(&self, header: &PacketHeader, message: &Message, target: &SocketAddr) -> io::Result<usize> {
        let encoded = MessageCodec::encode(header, message)?;
        self.transport.send(&encoded, target).await
    }

    pub async fn recv_message(&self) -> io::Result<(PacketHeader, Message, SocketAddr)> {
        let mut buf = vec![0; 1024]; // TODO: Dynamic buffer size
        let (len, addr) = self.transport.recv(&mut buf).await?;
        let (header, message) = MessageCodec::decode(&buf[..len])?;
        Ok((header, message, addr))
    }
}