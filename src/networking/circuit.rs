use crate::networking::transport::UdpTransport;
use std::net::SocketAddr;

pub struct Circuit {
    transport: UdpTransport,
    server_addr: SocketAddr,
}

impl Circuit {
    pub async fn new(server_addr: SocketAddr) -> std::io::Result<Self> {
        let transport = UdpTransport::new("0.0.0.0:0").await?;
        Ok(Self { transport, server_addr })
    }
}
