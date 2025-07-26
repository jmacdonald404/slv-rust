use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use bytes::BytesMut;
use std::net::SocketAddr;
use std::io;
use crate::utils::lludp::LluPacket;
use async_trait::async_trait;
use crate::ui::proxy::ProxySettings;
use crate::networking::socks5_udp::Socks5UdpSocket;

/// Minimal UDP message parser for Second Life protocol
pub fn parse_message_id(packet: &[u8]) -> Option<u16> {
    // LLUDP messages start with a 2-byte message ID (little-endian)
    if packet.len() >= 2 {
        Some(u16::from_le_bytes([packet[0], packet[1]]))
    } else {
        None
    }
}

#[async_trait]
pub trait UdpSocketExt: Send + Sync {
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> io::Result<usize>;
    async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

#[async_trait]
impl UdpSocketExt for UdpSocket {
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        UdpSocket::send_to(self, buf, target).await
    }
    async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, buf).await
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        UdpSocket::local_addr(self)
    }
}

pub struct UdpTransport {
    socket: std::sync::Arc<dyn UdpSocketExt>,
    sim_addr: SocketAddr,
    packet_id_counter: u32,
}

impl UdpTransport {
    pub async fn new(local_port: u16, sim_addr: SocketAddr, proxy_settings: Option<&ProxySettings>) -> io::Result<Self> {
        if let Some(proxy) = proxy_settings {
            if proxy.enabled {
                // Use SOCKS5 proxy, bind to the specified local_port
                let socks5 = Socks5UdpSocket::connect(&proxy.socks5_host, proxy.socks5_port, Some(local_port)).await?;
                let arc_socket: std::sync::Arc<dyn UdpSocketExt> = std::sync::Arc::new(socks5);
                return UdpTransport::new_with_socket(arc_socket, sim_addr, 1).await;
            }
        }
        // Use direct UDP socket, bind to the specified local_port
        let bind_addr = format!("0.0.0.0:{}", local_port);
        let socket = tokio::net::UdpSocket::bind(&bind_addr).await?;
        println!("[DEBUG] UDP socket bound to {}", socket.local_addr().unwrap());
        // TEMP: Send a test UDP packet to 127.0.0.1:54322
        let test_addr = "127.0.0.1:54322".parse::<SocketAddr>().unwrap();
        let test_msg = b"slv-rust authflow test";
        match socket.send_to(test_msg, &test_addr).await {
            Ok(sent) => println!("[TEMP TEST] Sent {} bytes to {} from auth flow UDP socket", sent, test_addr),
            Err(e) => println!("[TEMP TEST] UDP send error in auth flow: {}", e),
        }
        let arc_socket: std::sync::Arc<dyn UdpSocketExt> = std::sync::Arc::new(socket);
        UdpTransport::new_with_socket(arc_socket, sim_addr, 1).await // Start packet_id_counter at 1 // Start packet_id_counter at 1
    }

    pub async fn new_with_socket(socket: std::sync::Arc<dyn UdpSocketExt>, sim_addr: SocketAddr, initial_packet_id: u32) -> io::Result<Self> {
        Ok(UdpTransport { socket, sim_addr, packet_id_counter: initial_packet_id })
    }


    /// Log incoming LLUDP packets (for UseCircuitCode response and others)
    pub async fn recv_lludp_packet(&mut self, timeout_ms: u64) -> std::io::Result<Option<(LluPacket, std::net::SocketAddr)>> {
        let mut buf = BytesMut::with_capacity(1500);
        buf.resize(1500, 0);
        match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), self.socket.recv_from(&mut buf)).await {
            Ok(Ok((len, addr))) => {
                buf.truncate(len);
                if let Some(pkt) = LluPacket::parse_incoming(&buf) {
                    println!("[LLUDP IN] msg_id=0x{:04X} seq={:?} from {}: {:02X?}", pkt.message_id, pkt.sequence, addr, &buf);
                    if let Some(seq) = pkt.sequence {
                        if seq >= self.packet_id_counter {
                            self.packet_id_counter = seq + 1;
                        }
                    }
                    Ok(Some((pkt, addr)))
                } else {
                    println!("[LLUDP IN] Unparsed UDP packet ({} bytes) from {}: {:02X?}", buf.len(), addr, &buf);
                    Ok(None)
                }
            },
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(None), // timeout
        }
    }

    pub async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> std::io::Result<usize> {
        if buf.len() < 7 {
            println!("[UDP OUT] WARNING: Attempted to send packet < 7 bytes ({} bytes): {:02X?}", buf.len(), buf);
            return Ok(0);
        }
        println!("[UdpTransport] send_to: target = {}", target);
        // Log hex and ASCII
        fn to_hex_ascii(data: &[u8]) -> (String, String) {
            let hex = data.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            let ascii = data.iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect::<String>();
            (hex, ascii)
        }
        let (hex, ascii) = to_hex_ascii(buf);
        println!("[UDP OUT] To {} ({} bytes):\nHEX:   {}\nASCII: {}", target, buf.len(), hex, ascii);
        self.socket.send_to(buf, target).await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf).await
    }

    pub async fn recv_packet(&self, timeout_ms: u64) -> Result<Option<(Vec<u8>, SocketAddr)>, String> {
        println!("[DEBUG] Waiting for UDP response on socket {}...", self.socket.local_addr().unwrap());
        let mut buf = vec![0u8; 2048];
        let recv_result = timeout(Duration::from_millis(timeout_ms), self.socket.recv_from(&mut buf)).await;
        match recv_result {
            Ok(Ok((len, addr))) => {
                println!("[DEBUG] UDP receive returned {} bytes from {}", len, addr);
                buf.truncate(len);
                Ok(Some((buf, addr)))
            }
            Ok(Err(e)) => {
                println!("[DEBUG] UDP receive error: {}", e);
                Err(format!("UDP receive error: {e}"))
            }
            Err(_) => {
                println!("[DEBUG] UDP receive timed out after {} ms", timeout_ms);
                Ok(None)
            }
        }
    }

    // pub fn local_port(&self) -> u16 {
    //     self.socket.local_addr().map(|a| a.port()).unwrap_or(0)
    // }
}

// Example usage (to be called from your main or session logic):
//
// let sim_addr = "1.2.3.4:9000".parse().unwrap();
// let udp = UdpTransport::new(0, sim_addr).await?;
// let ucc = UseCircuitCode { circuit_code, session_id, agent_id };
// udp.send_use_circuit_code(&ucc).await?;
// if let Some((packet, addr)) = udp.recv_packet(1000).await? {
//     println!("[UDP] Received {} bytes from {}", packet.len(), addr);
// }
