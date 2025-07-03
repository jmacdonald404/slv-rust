use tokio::net::UdpSocket;
use std::io;

pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    pub async fn new(addr: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self { socket })
    }
}
