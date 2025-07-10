use tokio::net::UdpSocket;
use std::io;
use std::net::SocketAddr;

pub struct UdpTransport {
    pub socket: UdpSocket,
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

pub trait UdpSocketExt: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn local_addr(&self) -> io::Result<SocketAddr>;
    fn boxed(self) -> Box<dyn UdpSocketExt>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
    fn send<'a>(&'a self, buf: &'a [u8], target: &'a SocketAddr) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<usize>> + Send + 'a>>;
    fn recv<'a>(&'a self, buf: &'a mut [u8]) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<(usize, SocketAddr)>> + Send + 'a>>;
}

impl UdpSocketExt for UdpTransport {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
    fn send<'a>(&'a self, buf: &'a [u8], target: &'a SocketAddr) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<usize>> + Send + 'a>> {
        Box::pin(self.socket.send_to(buf, target))
    }
    fn recv<'a>(&'a self, buf: &'a mut [u8]) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<(usize, SocketAddr)>> + Send + 'a>> {
        Box::pin(self.socket.recv_from(buf))
    }
}
