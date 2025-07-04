use tokio::net::UdpSocket;
use std::io;
use std::net::SocketAddr;

pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    pub async fn new(addr: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        println!("UdpTransport bound to: {}", socket.local_addr()?);
        Ok(Self { socket })
    }

    pub async fn send(&self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        self.socket.send_to(buf, target).await
    }

    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf).await
    }
}
