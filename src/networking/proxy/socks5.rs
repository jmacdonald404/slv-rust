//! SOCKS5 UDP proxy client implementation
//! 
//! Based on RFC 1928 (SOCKS Protocol Version 5) and Hippolyzer requirements:
//! - Maintains a TCP connection for UDP association
//! - Encapsulates UDP packets with SOCKS5 headers
//! - Handles authentication and connection management

use crate::networking::{NetworkError, NetworkResult};
use bytes::{Bytes, BytesMut, BufMut};
use std::net::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock, Semaphore};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

impl Drop for Socks5UdpClient {
    fn drop(&mut self) {
        info!("🗑️ Dropping SOCKS5 client for {}", self.proxy_addr);
    }
}

/// SOCKS5 command codes
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Socks5Command {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x03,
}

/// SOCKS5 address types
#[repr(u8)] 
#[derive(Debug, Clone, Copy)]
pub enum Socks5AddressType {
    Ipv4 = 0x01,
    DomainName = 0x03,
    Ipv6 = 0x04,
}

/// SOCKS5 authentication methods
#[repr(u8)]
#[derive(Debug, Clone, Copy)]  
pub enum Socks5AuthMethod {
    NoAuth = 0x00,
    Gssapi = 0x01,
    UsernamePassword = 0x02,
    NoAcceptableMethods = 0xFF,
}

/// SOCKS5 reply codes
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Socks5ReplyCode {
    Success = 0x00,
    GeneralFailure = 0x01,
    ConnectionNotAllowed = 0x02, 
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddressTypeNotSupported = 0x08,
}

/// SOCKS5 UDP packet header
#[derive(Debug, Clone)]
pub struct Socks5UdpHeader {
    pub fragment: u8,
    pub address_type: Socks5AddressType,
    pub address: IpAddr,
    pub port: u16,
}

impl Socks5UdpHeader {
    /// Create a new UDP header
    pub fn new(socket_addr: SocketAddr) -> Self {
        let (address_type, address) = match socket_addr.ip() {
            IpAddr::V4(ipv4) => (Socks5AddressType::Ipv4, IpAddr::V4(ipv4)),
            IpAddr::V6(ipv6) => (Socks5AddressType::Ipv6, IpAddr::V6(ipv6)),
        };
        
        Self {
            fragment: 0,
            address_type,
            address,
            port: socket_addr.port(),
        }
    }
    
    /// Serialize the header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // Reserved (2 bytes) + Fragment (1 byte)
        buf.extend_from_slice(&[0x00, 0x00, self.fragment]);
        
        // Address type
        buf.push(self.address_type as u8);
        
        // Address
        match self.address {
            IpAddr::V4(ipv4) => {
                buf.extend_from_slice(&ipv4.octets());
            }
            IpAddr::V6(ipv6) => {
                buf.extend_from_slice(&ipv6.octets());
            }
        }
        
        // Port (big-endian)
        buf.extend_from_slice(&self.port.to_be_bytes());
        
        buf
    }
    
    /// Parse header from bytes
    pub fn from_bytes(data: &[u8]) -> NetworkResult<(Self, usize)> {
        if data.len() < 6 {
            return Err(NetworkError::Transport {
                reason: "SOCKS5 UDP header too short".to_string()
            });
        }
        
        let fragment = data[2];
        let address_type = match data[3] {
            0x01 => Socks5AddressType::Ipv4,
            0x03 => Socks5AddressType::DomainName,
            0x04 => Socks5AddressType::Ipv6,
            _ => return Err(NetworkError::Transport {
                reason: "Invalid SOCKS5 address type".to_string()
            }),
        };
        
        let (address, port, header_len) = match address_type {
            Socks5AddressType::Ipv4 => {
                if data.len() < 10 {
                    return Err(NetworkError::Transport {
                        reason: "SOCKS5 IPv4 header too short".to_string()
                    });
                }
                let ipv4 = Ipv4Addr::new(data[4], data[5], data[6], data[7]);
                let port = u16::from_be_bytes([data[8], data[9]]);
                (IpAddr::V4(ipv4), port, 10)
            }
            Socks5AddressType::Ipv6 => {
                if data.len() < 22 {
                    return Err(NetworkError::Transport {
                        reason: "SOCKS5 IPv6 header too short".to_string()
                    });
                }
                let mut ipv6_bytes = [0u8; 16];
                ipv6_bytes.copy_from_slice(&data[4..20]);
                let ipv6 = Ipv6Addr::from(ipv6_bytes);
                let port = u16::from_be_bytes([data[20], data[21]]);
                (IpAddr::V6(ipv6), port, 22)
            }
            Socks5AddressType::DomainName => {
                return Err(NetworkError::Transport {
                    reason: "SOCKS5 domain names not supported".to_string()
                });
            }
        };
        
        let header = Self {
            fragment,
            address_type,
            address,
            port,
        };
        
        Ok((header, header_len))
    }
    
    /// Get the socket address
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.address, self.port)
    }
}

/// SOCKS5 UDP proxy client
pub struct Socks5UdpClient {
    /// Proxy server address
    proxy_addr: SocketAddr,
    /// TCP control connection to proxy
    control_stream: Arc<Mutex<Option<TcpStream>>>,
    /// UDP relay address provided by proxy
    relay_addr: Arc<RwLock<Option<SocketAddr>>>,
    /// Local UDP socket for sending/receiving
    udp_socket: Arc<Mutex<Option<UdpSocket>>>,
    /// Authentication credentials
    username: Option<String>,
    password: Option<String>,
    /// Connection semaphore to prevent concurrent connections
    connection_semaphore: Arc<Semaphore>,
}

impl Socks5UdpClient {
    /// Create a new SOCKS5 UDP client
    pub fn new(proxy_addr: SocketAddr, username: Option<String>, password: Option<String>) -> Self {
        Self {
            proxy_addr,
            control_stream: Arc::new(Mutex::new(None)),
            relay_addr: Arc::new(RwLock::new(None)),
            udp_socket: Arc::new(Mutex::new(None)),
            username,
            password,
            connection_semaphore: Arc::new(Semaphore::new(1)), // Only allow one connection at a time
        }
    }
    
    /// Connect to the SOCKS5 proxy and establish UDP association
    pub async fn connect(&self) -> NetworkResult<()> {
        // Acquire semaphore to prevent concurrent connections
        let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
            NetworkError::Transport {
                reason: "Failed to acquire connection semaphore".to_string()
            }
        })?;
        
        info!("🔗 SOCKS5 connect() called for proxy {}", self.proxy_addr);
        
        // Check if already connected to avoid duplicate connections
        if self.is_connected().await {
            info!("✅ Already connected to SOCKS5 proxy, skipping connection");
            return Ok(());
        }
        
        info!("🆕 Establishing new SOCKS5 connection...");
        
        // Connect TCP control stream with timeout
        let mut stream = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            TcpStream::connect(self.proxy_addr)
        ).await
        .map_err(|_| NetworkError::Transport {
            reason: "Timeout connecting to SOCKS5 proxy".to_string()
        })??;
        
        // Set TCP keepalive to maintain control connection
        if let Err(e) = stream.set_nodelay(true) {
            warn!("Failed to set TCP_NODELAY: {}", e);
        }
        
        info!("TCP control connection established to {}", self.proxy_addr);
        
        // Step 1: Authentication negotiation
        self.negotiate_auth(&mut stream).await?;
        info!("SOCKS5 authentication completed");
        
        // Step 2: UDP Associate request
        let relay_addr = self.udp_associate(&mut stream).await?;
        info!("SOCKS5 UDP association completed, relay: {}", relay_addr);
        
        // Create local UDP socket bound to localhost for better compatibility
        let udp_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let local_udp_addr = udp_socket.local_addr()?;
        info!("Local UDP socket bound to {}", local_udp_addr);
        
        // Store everything atomically
        {
            let mut control_guard = self.control_stream.lock().await;
            *control_guard = Some(stream);
        }
        
        {
            let mut relay_guard = self.relay_addr.write().await;
            *relay_guard = Some(relay_addr);
        }
        
        {
            let mut socket_guard = self.udp_socket.lock().await;
            *socket_guard = Some(udp_socket);
        }
        
        // Start keep-alive task for TCP control connection
        self.start_keepalive_task().await;
        
        info!("✅ SOCKS5 UDP association fully established");
        Ok(())
    }
    
    /// Negotiate authentication with proxy
    async fn negotiate_auth(&self, stream: &mut TcpStream) -> NetworkResult<()> {
        // Send authentication methods
        let methods = if self.username.is_some() && self.password.is_some() {
            vec![Socks5AuthMethod::NoAuth as u8, Socks5AuthMethod::UsernamePassword as u8]
        } else {
            vec![Socks5AuthMethod::NoAuth as u8]
        };
        
        let mut request = vec![0x05, methods.len() as u8]; // SOCKS5 version + method count
        request.extend(methods);
        
        debug!("Sending SOCKS5 auth negotiation: {:02x?}", request);
        stream.write_all(&request).await?;
        stream.flush().await?; // Ensure data is sent immediately
        
        // Read server response
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 {
            return Err(NetworkError::Transport {
                reason: "Invalid SOCKS5 version in auth response".to_string()
            });
        }
        
        let selected_method = response[1];
        match selected_method {
            x if x == Socks5AuthMethod::NoAuth as u8 => {
                debug!("SOCKS5 authentication: No auth required");
                Ok(())
            }
            x if x == Socks5AuthMethod::UsernamePassword as u8 => {
                debug!("SOCKS5 authentication: Username/password required");
                self.authenticate_username_password(stream).await
            }
            x if x == Socks5AuthMethod::NoAcceptableMethods as u8 => {
                Err(NetworkError::Transport {
                    reason: "SOCKS5 proxy rejected all authentication methods".to_string()
                })
            }
            _ => {
                Err(NetworkError::Transport {
                    reason: format!("SOCKS5 proxy selected unsupported auth method: {}", selected_method)
                })
            }
        }
    }
    
    /// Perform username/password authentication
    async fn authenticate_username_password(&self, stream: &mut TcpStream) -> NetworkResult<()> {
        let username = self.username.as_ref().ok_or_else(|| NetworkError::Transport {
            reason: "Username required for SOCKS5 authentication".to_string()
        })?;
        let password = self.password.as_ref().ok_or_else(|| NetworkError::Transport {
            reason: "Password required for SOCKS5 authentication".to_string()
        })?;
        
        let mut request = vec![0x01]; // Username/password auth version
        request.push(username.len() as u8);
        request.extend(username.as_bytes());
        request.push(password.len() as u8);
        request.extend(password.as_bytes());
        
        stream.write_all(&request).await?;
        
        // Read authentication response
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x01 {
            return Err(NetworkError::Transport {
                reason: "Invalid username/password auth version".to_string()
            });
        }
        
        if response[1] != 0x00 {
            return Err(NetworkError::Transport {
                reason: "SOCKS5 username/password authentication failed".to_string()
            });
        }
        
        debug!("SOCKS5 username/password authentication successful");
        Ok(())
    }
    
    /// Send UDP Associate command
    async fn udp_associate(&self, stream: &mut TcpStream) -> NetworkResult<SocketAddr> {
        // Build UDP Associate request
        let mut request = vec![
            0x05, // SOCKS version
            Socks5Command::UdpAssociate as u8, // Command
            0x00, // Reserved
            Socks5AddressType::Ipv4 as u8, // Address type
        ];
        
        // Client's UDP address (127.0.0.1:0 for localhost binding)
        request.extend_from_slice(&[127, 0, 0, 1]); // 127.0.0.1
        request.extend_from_slice(&[0, 0]); // Port 0
        
        debug!("Sending SOCKS5 UDP associate request: {:02x?}", request);
        stream.write_all(&request).await?;
        stream.flush().await?; // Ensure data is sent immediately
        
        // Read response
        let mut response = vec![0u8; 4];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 {
            return Err(NetworkError::Transport {
                reason: "Invalid SOCKS5 version in UDP associate response".to_string()
            });
        }
        
        let reply_code = response[1];
        if reply_code != Socks5ReplyCode::Success as u8 {
            return Err(NetworkError::Transport {
                reason: format!("SOCKS5 UDP associate failed with code: {}", reply_code)
            });
        }
        
        let address_type = response[3];
        let relay_addr = match address_type {
            x if x == Socks5AddressType::Ipv4 as u8 => {
                let mut addr_buf = [0u8; 6]; // 4 bytes IP + 2 bytes port
                stream.read_exact(&mut addr_buf).await?;
                let ip = Ipv4Addr::new(addr_buf[0], addr_buf[1], addr_buf[2], addr_buf[3]);
                let port = u16::from_be_bytes([addr_buf[4], addr_buf[5]]);
                
                // If proxy returns 0.0.0.0, replace with proxy's actual IP
                let final_ip = if ip.is_unspecified() {
                    debug!("Proxy returned 0.0.0.0, using proxy IP instead");
                    match self.proxy_addr.ip() {
                        IpAddr::V4(proxy_ip) => proxy_ip,
                        IpAddr::V6(_) => {
                            return Err(NetworkError::Transport {
                                reason: "IPv6 proxy with IPv4 relay address not supported".to_string()
                            });
                        }
                    }
                } else {
                    ip
                };
                
                SocketAddr::new(IpAddr::V4(final_ip), port)
            }
            x if x == Socks5AddressType::Ipv6 as u8 => {
                let mut addr_buf = [0u8; 18]; // 16 bytes IP + 2 bytes port
                stream.read_exact(&mut addr_buf).await?;
                let mut ip_bytes = [0u8; 16];
                ip_bytes.copy_from_slice(&addr_buf[0..16]);
                let ip = Ipv6Addr::from(ip_bytes);
                let port = u16::from_be_bytes([addr_buf[16], addr_buf[17]]);
                
                // If proxy returns ::, replace with proxy's actual IP
                let final_ip = if ip.is_unspecified() {
                    debug!("Proxy returned ::, using proxy IP instead");
                    match self.proxy_addr.ip() {
                        IpAddr::V6(proxy_ip) => proxy_ip,
                        IpAddr::V4(_) => {
                            return Err(NetworkError::Transport {
                                reason: "IPv4 proxy with IPv6 relay address not supported".to_string()
                            });
                        }
                    }
                } else {
                    ip
                };
                
                SocketAddr::new(IpAddr::V6(final_ip), port)
            }
            _ => {
                return Err(NetworkError::Transport {
                    reason: "Unsupported address type in SOCKS5 UDP associate response".to_string()
                });
            }
        };
        
        Ok(relay_addr)
    }
    
    /// Send UDP packet through SOCKS5 proxy
    pub async fn send_to(&self, data: &[u8], target: SocketAddr) -> NetworkResult<()> {
        // Skip connection check to avoid potential deadlock - assume connection is active
        // The actual send will fail if connection is lost anyway
        info!("🚀 SOCKS5 SEND_TO: Starting send operation to {}", target);
        
        let relay_addr = {
            // Try to get relay address with timeout to avoid hanging
            let relay_guard = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                self.relay_addr.read()
            ).await;
            
            match relay_guard {
                Ok(guard) => guard.ok_or_else(|| NetworkError::Transport {
                    reason: "SOCKS5 UDP association not established".to_string()
                })?,
                Err(_) => {
                    return Err(NetworkError::Transport {
                        reason: "Timeout getting SOCKS5 relay address - possible deadlock".to_string()
                    });
                }
            }
        };
        
        let socket_guard = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            self.udp_socket.lock()
        ).await.map_err(|_| NetworkError::Transport {
            reason: "Timeout acquiring UDP socket lock - possible contention".to_string()
        })?;
        
        let socket = socket_guard.as_ref().ok_or_else(|| NetworkError::Transport {
            reason: "UDP socket not available".to_string()
        })?;
        
        // Create SOCKS5 UDP header
        let header = Socks5UdpHeader::new(target);
        let header_bytes = header.to_bytes();
        
        // Combine header and data
        let mut packet = BytesMut::with_capacity(header_bytes.len() + data.len());
        packet.put_slice(&header_bytes);
        packet.put_slice(data);
        
        // Send to proxy relay address with error handling
        info!("🌐 SOCKS5 SEND_TO: Sending {} bytes (+ {} header) to proxy relay {}", data.len(), header_bytes.len(), relay_addr);
        info!("🎯 SOCKS5 SEND_TO: Target destination: {}", target);
        match socket.send_to(&packet, relay_addr).await {
            Ok(bytes_sent) => {
                if bytes_sent != packet.len() {
                    warn!("Partial SOCKS5 send: {} of {} bytes", bytes_sent, packet.len());
                }
                debug!("📤 Sent {} bytes through SOCKS5 proxy to {} (via {})", data.len(), target, relay_addr);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send UDP packet through SOCKS5 proxy: {}", e);
                Err(NetworkError::Transport {
                    reason: format!("SOCKS5 UDP send failed: {}", e)
                })
            }
        }
    }
    
    /// Receive UDP packet through SOCKS5 proxy
    pub async fn recv_from(&self, buf: &mut [u8]) -> NetworkResult<(usize, SocketAddr)> {
        let socket_guard = self.udp_socket.lock().await;
        let socket = socket_guard.as_ref().ok_or_else(|| NetworkError::Transport {
            reason: "UDP socket not available".to_string()
        })?;
        
        // Receive from proxy
        let (len, _proxy_addr) = socket.recv_from(buf).await?;
        
        // Parse SOCKS5 UDP header
        let (header, header_len) = Socks5UdpHeader::from_bytes(&buf[..len])?;
        
        // Move data to beginning of buffer
        let data_len = len - header_len;
        buf.copy_within(header_len..len, 0);
        
        debug!("Received {} bytes through SOCKS5 proxy from {}", data_len, header.socket_addr());
        Ok((data_len, header.socket_addr()))
    }
    
    /// Get the local UDP socket address
    pub async fn local_addr(&self) -> NetworkResult<SocketAddr> {
        let socket_guard = self.udp_socket.lock().await;
        let socket = socket_guard.as_ref().ok_or_else(|| NetworkError::Transport {
            reason: "UDP socket not available".to_string()
        })?;
        
        Ok(socket.local_addr()?)
    }
    
    /// Check if connected to proxy
    pub async fn is_connected(&self) -> bool {
        let control_guard = self.control_stream.lock().await;
        let relay_guard = self.relay_addr.read().await;
        control_guard.is_some() && relay_guard.is_some()
    }
    
    /// Start keep-alive task for TCP control connection
    async fn start_keepalive_task(&self) {
        let control_stream = Arc::clone(&self.control_stream);
        let proxy_addr = self.proxy_addr;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Check if connection is still alive
                let should_continue = {
                    let guard = control_stream.lock().await;
                    guard.is_some()
                };
                
                if !should_continue {
                    debug!("Keep-alive task stopping - connection closed");
                    break;
                }
                
                debug!("SOCKS5 control connection keep-alive check for {}", proxy_addr);
            }
        });
    }
    
    /// Disconnect from proxy
    pub async fn disconnect(&self) -> NetworkResult<()> {
        info!("🔌 Disconnecting from SOCKS5 proxy {}...", self.proxy_addr);
        
        {
            let mut control_guard = self.control_stream.lock().await;
            if let Some(mut stream) = control_guard.take() {
                info!("Shutting down TCP control connection");
                let _ = stream.shutdown().await;
            }
        }
        
        {
            let mut relay_guard = self.relay_addr.write().await;
            *relay_guard = None;
        }
        
        {
            let mut socket_guard = self.udp_socket.lock().await;
            *socket_guard = None;
        }
        
        info!("✅ Disconnected from SOCKS5 proxy");
        Ok(())
    }
}