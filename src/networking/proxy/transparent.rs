//! Transparent SOCKS5 proxy implementation
//! 
//! This module implements WinHippoAutoProxy-like functionality directly in slv-rust.
//! Instead of using API hooking, we transparently wrap/unwrap SOCKS5 headers
//! at the transport layer.

use std::net::SocketAddr;
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{debug, error, info, warn};
use crate::networking::{NetworkError, NetworkResult};

/// Transparent SOCKS5 proxy handler that mimics WinHippoAutoProxy behavior
pub struct TransparentSocks5Proxy {
    /// SOCKS5 proxy address (e.g., 127.0.0.1:9061)
    proxy_addr: SocketAddr,
    /// Local UDP socket for sending SOCKS5-wrapped packets
    local_socket: Option<std::net::UdpSocket>,
}

impl TransparentSocks5Proxy {
    /// Create a new transparent SOCKS5 proxy handler
    pub fn new(proxy_addr: SocketAddr) -> Self {
        Self {
            proxy_addr,
            local_socket: None,
        }
    }
    
    /// Initialize the transparent proxy by binding a local UDP socket
    pub async fn initialize(&mut self) -> NetworkResult<()> {
        // Bind to localhost for better compatibility with proxy software
        let socket = std::net::UdpSocket::bind("127.0.0.1:0")
            .map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to bind local socket for transparent proxy: {}", e) 
            })?;
            
        let local_addr = socket.local_addr()
            .map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to get local address: {}", e) 
            })?;
        
        // Set socket timeout to prevent hanging
        socket.set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .map_err(|e| NetworkError::Transport {
                reason: format!("Failed to set socket read timeout: {}", e)
            })?;
        
        socket.set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| NetworkError::Transport {
                reason: format!("Failed to set socket write timeout: {}", e)
            })?;
            
        info!("🔧 Transparent SOCKS5 proxy initialized on local port {}", local_addr.port());
        self.local_socket = Some(socket);
        Ok(())
    }
    
    /// Transparently send a UDP packet through SOCKS5 proxy
    /// This mimics what WinHippoAutoProxy does with API hooking
    pub async fn send_transparent(&self, data: &[u8], dest_addr: SocketAddr) -> NetworkResult<()> {
        let socket = self.local_socket.as_ref()
            .ok_or_else(|| NetworkError::Transport { 
                reason: "Transparent proxy not initialized".to_string() 
            })?;
            
        // Create SOCKS5 UDP packet with header
        let socks5_packet = self.wrap_with_socks5_header(data, dest_addr)?;
        
        debug!("📤 Sending {} bytes + {} header bytes = {} total to SOCKS5 proxy {}", 
               data.len(), socks5_packet.len() - data.len(), socks5_packet.len(), self.proxy_addr);
        
        // Send the wrapped packet to the SOCKS5 proxy with error handling
        match socket.send_to(&socks5_packet, self.proxy_addr) {
            Ok(bytes_sent) => {
                if bytes_sent != socks5_packet.len() {
                    warn!("Partial transparent SOCKS5 send: {} of {} bytes", bytes_sent, socks5_packet.len());
                }
                debug!("📤 Sent {} bytes transparently through SOCKS5 proxy to {}", data.len(), dest_addr);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send to SOCKS5 proxy {}: {}", self.proxy_addr, e);
                Err(NetworkError::Transport { 
                    reason: format!("Failed to send to SOCKS5 proxy: {}", e) 
                })
            }
        }
    }
    
    /// Wrap UDP data with SOCKS5 header for proxy transmission
    fn wrap_with_socks5_header(&self, data: &[u8], dest_addr: SocketAddr) -> NetworkResult<Vec<u8>> {
        let mut packet = BytesMut::new();
        
        // SOCKS5 UDP packet format:
        // +----+------+------+----------+----------+----------+
        // |RSV | FRAG | ATYP | DST.ADDR | DST.PORT |   DATA   |
        // +----+------+------+----------+----------+----------+
        // | 2  |  1   |  1   | Variable |    2     | Variable |
        // +----+------+------+----------+----------+----------+
        
        // Reserved (2 bytes)
        packet.put_u16(0);
        
        // Fragment (1 byte) - 0 means not fragmented
        packet.put_u8(0);
        
        // Address type and address
        match dest_addr {
            SocketAddr::V4(addr) => {
                // IPv4 address type
                packet.put_u8(0x01);
                // IPv4 address (4 bytes)
                packet.put_slice(&addr.ip().octets());
                // Port (2 bytes)
                packet.put_u16(addr.port());
            }
            SocketAddr::V6(addr) => {
                // IPv6 address type
                packet.put_u8(0x04);
                // IPv6 address (16 bytes)
                packet.put_slice(&addr.ip().octets());
                // Port (2 bytes)
                packet.put_u16(addr.port());
            }
        }
        
        // Actual UDP data
        packet.put_slice(data);
        
        Ok(packet.to_vec())
    }
    
    /// Unwrap SOCKS5 header from received UDP data
    /// This would be used if we were receiving data back through the proxy
    pub fn unwrap_socks5_header(&self, data: &[u8]) -> NetworkResult<(Bytes, SocketAddr)> {
        if data.len() < 10 {
            return Err(NetworkError::PacketDecode { 
                reason: "SOCKS5 UDP packet too short".to_string() 
            });
        }
        
        let mut offset = 0;
        
        // Skip reserved (2 bytes)
        offset += 2;
        
        // Skip fragment (1 byte)  
        offset += 1;
        
        // Address type
        let atyp = data[offset];
        offset += 1;
        
        let (addr, port) = match atyp {
            0x01 => {
                // IPv4
                if data.len() < offset + 6 {
                    return Err(NetworkError::PacketDecode { 
                        reason: "Invalid IPv4 address in SOCKS5 packet".to_string() 
                    });
                }
                let ip_bytes = &data[offset..offset + 4];
                let ip = std::net::Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                offset += 4;
                let port = u16::from_be_bytes([data[offset], data[offset + 1]]);
                offset += 2;
                (std::net::IpAddr::V4(ip), port)
            }
            0x04 => {
                // IPv6
                if data.len() < offset + 18 {
                    return Err(NetworkError::PacketDecode { 
                        reason: "Invalid IPv6 address in SOCKS5 packet".to_string() 
                    });
                }
                let ip_bytes: [u8; 16] = data[offset..offset + 16].try_into()
                    .map_err(|_| NetworkError::PacketDecode { 
                        reason: "Failed to parse IPv6 address".to_string() 
                    })?;
                let ip = std::net::Ipv6Addr::from(ip_bytes);
                offset += 16;
                let port = u16::from_be_bytes([data[offset], data[offset + 1]]);
                offset += 2;
                (std::net::IpAddr::V6(ip), port)
            }
            _ => {
                return Err(NetworkError::PacketDecode { 
                    reason: format!("Unsupported address type in SOCKS5 packet: {}", atyp) 
                });
            }
        };
        
        let source_addr = SocketAddr::new(addr, port);
        let payload = Bytes::copy_from_slice(&data[offset..]);
        
        debug!("📥 Unwrapped SOCKS5 packet from {}, {} bytes", source_addr, payload.len());
        Ok((payload, source_addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};
    
    #[test]
    fn test_socks5_header_wrapping() {
        let proxy = TransparentSocks5Proxy::new("127.0.0.1:9061".parse().unwrap());
        let test_data = b"Hello, world!";
        let dest_addr = SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        let wrapped = proxy.wrap_with_socks5_header(test_data, dest_addr).unwrap();
        
        // Check SOCKS5 header structure
        assert_eq!(wrapped[0], 0); // Reserved byte 1
        assert_eq!(wrapped[1], 0); // Reserved byte 2
        assert_eq!(wrapped[2], 0); // Fragment
        assert_eq!(wrapped[3], 1); // IPv4 address type
        assert_eq!(&wrapped[4..8], &[192, 168, 1, 1]); // IP address
        assert_eq!(u16::from_be_bytes([wrapped[8], wrapped[9]]), 8080); // Port
        assert_eq!(&wrapped[10..], test_data); // Original data
    }
    
    #[test]
    fn test_socks5_header_unwrapping() {
        let proxy = TransparentSocks5Proxy::new("127.0.0.1:9061".parse().unwrap());
        
        // Create a test SOCKS5 packet
        let mut packet = vec![
            0, 0, // Reserved
            0,    // Fragment
            1,    // IPv4 address type
            192, 168, 1, 1, // IP address
            0x1f, 0x90, // Port 8080 in big-endian
        ];
        packet.extend_from_slice(b"Hello, world!");
        
        let (payload, source_addr) = proxy.unwrap_socks5_header(&packet).unwrap();
        
        assert_eq!(payload.as_ref(), b"Hello, world!");
        assert_eq!(source_addr.ip(), std::net::IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(source_addr.port(), 8080);
    }
}