use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::io;
use tracing::{info, error, debug};
use async_trait::async_trait;

#[async_trait]
pub trait UdpSocketExt {
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> std::io::Result<usize>;
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)>;
    fn local_addr(&self) -> std::io::Result<SocketAddr>;
}

pub struct Socks5UdpSocket {
    pub udp_socket: UdpSocket,
    pub relay_addr: SocketAddr,
    _tcp_stream: TcpStream, // Keep this alive!
}

impl Socks5UdpSocket {
    pub async fn connect(proxy_host: &str, proxy_port: u16, local_port: Option<u16>) -> io::Result<Self> {
        let proxy_addr = format!("{}:{}", proxy_host, proxy_port);
        info!("[SOCKS5] Connecting to SOCKS5 proxy at {}", proxy_addr);
        let mut tcp_stream = TcpStream::connect(&proxy_addr).await?;

        // SOCKS5 handshake (no auth)
        tcp_stream.write_all(&[0x05, 0x01, 0x00]).await?;
        let mut resp = [0u8; 2];
        tcp_stream.read_exact(&mut resp).await?;
        if resp != [0x05, 0x00] {
            error!("[SOCKS5] SOCKS5 handshake failed: {:?}", resp);
            return Err(io::Error::new(io::ErrorKind::Other, "SOCKS5 handshake failed"));
        }
        debug!("[SOCKS5] SOCKS5 handshake succeeded");

        // UDP ASSOCIATE
        let bind_addr = match local_port {
            Some(port) => format!("127.0.0.1:{}", port),
            None => "127.0.0.1:0".to_string(),
        };
        let local_udp = UdpSocket::bind(&bind_addr).await?;
        let local_addr = local_udp.local_addr()?;
        info!("[SOCKS5] Local UDP socket bound to {}", local_addr);
        let local_ip = match local_addr.ip() {
            IpAddr::V4(ip) => ip.octets(),
            IpAddr::V6(_) => {
                error!("[SOCKS5] IPv6 not supported for SOCKS5 UDP");
                return Err(io::Error::new(io::ErrorKind::Other, "IPv6 not supported"));
            }
        };
        let local_port = local_addr.port();
        let mut req = vec![0x05, 0x03, 0x00, 0x01];
        req.extend_from_slice(&local_ip);
        req.extend_from_slice(&local_port.to_be_bytes());
        tcp_stream.write_all(&req).await?;

        // Parse response
        let mut resp = [0u8; 4];
        tcp_stream.read_exact(&mut resp).await?;
        if resp[0] != 0x05 || resp[1] != 0x00 {
            error!("[SOCKS5] SOCKS5 UDP associate failed: {:?}", resp);
            return Err(io::Error::new(io::ErrorKind::Other, "SOCKS5 UDP associate failed"));
        }
        let atyp = resp[3];
        let mut relay_addr = match atyp {
            0x01 => {
                let mut ip = [0u8; 4];
                tcp_stream.read_exact(&mut ip).await?;
                let mut port = [0u8; 2];
                tcp_stream.read_exact(&mut port).await?;
                SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), u16::from_be_bytes(port))
            }
            _ => {
                error!("[SOCKS5] Unsupported ATYP in UDP associate reply: {}", atyp);
                return Err(io::Error::new(io::ErrorKind::Other, "Unsupported ATYP in UDP associate reply"));
            }
        };
        
        // If the proxy returns 0.0.0.0, we should use the proxy's actual IP address.
        if relay_addr.ip().is_unspecified() {
            let proxy_ip = tcp_stream.peer_addr()?.ip();
            relay_addr.set_ip(proxy_ip);
            info!("[SOCKS5] Relay IP was unspecified, using proxy IP {} instead.", proxy_ip);
        }
        
        info!("[SOCKS5] SOCKS5 UDP relay address: {}", relay_addr);

        Ok(Self {
            udp_socket: local_udp,
            relay_addr,
            _tcp_stream: tcp_stream,
        })
    }

    pub fn build_udp_packet(data: &[u8], dest: &SocketAddr) -> Vec<u8> {
        let mut packet = Vec::with_capacity(10 + data.len());
        packet.extend_from_slice(&[0x00, 0x00, 0x00]); // RSV, FRAG
        match dest.ip() {
            IpAddr::V4(ip) => {
                packet.push(0x01);
                packet.extend_from_slice(&ip.octets());
            }
            IpAddr::V6(ip) => {
                packet.push(0x04);
                packet.extend_from_slice(&ip.octets());
            }
        }
        packet.extend_from_slice(&dest.port().to_be_bytes());
        packet.extend_from_slice(data);
        packet
    }

    fn parse_udp_packet<'a>(buf: &'a mut [u8], n: usize) -> io::Result<(usize, SocketAddr)> {
        if n < 10 {
            error!("SOCKS5 UDP packet too short: {} bytes", n);
            return Err(io::Error::new(io::ErrorKind::Other, "SOCKS5 UDP packet too short"));
        }
        let frag = buf[2];
        if frag != 0x00 {
            error!("SOCKS5 UDP fragmentation not supported");
            return Err(io::Error::new(io::ErrorKind::Other, "SOCKS5 UDP fragmentation not supported"));
        }
        let atyp = buf[3];
        let (addr, header_len) = match atyp {
            0x01 => {
                if n < 10 { return Err(io::Error::new(io::ErrorKind::Other, "SOCKS5 UDP IPv4 header too short")); }
                let ip = IpAddr::V4(Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]));
                let port = u16::from_be_bytes([buf[8], buf[9]]);
                (SocketAddr::new(ip, port), 10)
            }
            0x04 => {
                if n < 22 { return Err(io::Error::new(io::ErrorKind::Other, "SOCKS5 UDP IPv6 header too short")); }
                let ip = IpAddr::V6(std::net::Ipv6Addr::new(
                    u16::from_be_bytes([buf[4], buf[5]]),
                    u16::from_be_bytes([buf[6], buf[7]]),
                    u16::from_be_bytes([buf[8], buf[9]]),
                    u16::from_be_bytes([buf[10], buf[11]]),
                    u16::from_be_bytes([buf[12], buf[13]]),
                    u16::from_be_bytes([buf[14], buf[15]]),
                    u16::from_be_bytes([buf[16], buf[17]]),
                    u16::from_be_bytes([buf[18], buf[19]]),
                ));
                let port = u16::from_be_bytes([buf[20], buf[21]]);
                (SocketAddr::new(ip, port), 22)
            }
            _ => {
                error!("Unsupported ATYP in SOCKS5 UDP packet: {}", atyp);
                return Err(io::Error::new(io::ErrorKind::Other, "Unsupported ATYP in SOCKS5 UDP packet"));
            }
        };
        let data_len = n - header_len;
        buf.copy_within(header_len..n, 0);
        Ok((data_len, addr))
    }
}

#[async_trait]
impl UdpSocketExt for Socks5UdpSocket {
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> std::io::Result<usize> {
        info!("[SOCKS5] Actually sending UDP packet to relay {} (real dest: {})", self.relay_addr, target);
        let packet = Self::build_udp_packet(buf, target);
        self.udp_socket.send_to(&packet, self.relay_addr).await
    }
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        let (n, src) = self.udp_socket.recv_from(buf).await?;
        debug!("[SOCKS5] Received {} bytes from relay {}", n, src);
        match Self::parse_udp_packet(buf, n) {
            Ok((data_len, addr)) => Ok((data_len, addr)),
            Err(e) => Err(e),
        }
    }
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.udp_socket.local_addr()
    }
} 