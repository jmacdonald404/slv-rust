//! UDP transport layer with SOCKS5 proxy support
//! 
//! Handles the low-level UDP socket operations, including optional SOCKS5 proxy
//! routing for environments that require proxy connections.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::PacketWrapper;
use crate::networking::serialization::PacketDeserializer;
use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// SOCKS5 proxy configuration
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// UDP transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Local bind address (0.0.0.0:0 for any)
    pub bind_addr: SocketAddr,
    /// Optional SOCKS5 proxy configuration
    pub proxy: Option<ProxyConfig>,
    /// Maximum packet size
    pub max_packet_size: usize,
    /// Receive buffer size
    pub recv_buffer_size: usize,
    /// Send buffer size
    pub send_buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:0".parse().unwrap(),
            proxy: None,
            max_packet_size: 1500,
            recv_buffer_size: 64 * 1024,
            send_buffer_size: 64 * 1024,
        }
    }
}

/// UDP transport for Second Life protocol
pub struct UdpTransport {
    /// UDP socket
    socket: Arc<UdpSocket>,
    
    /// Transport configuration
    config: TransportConfig,
    
    /// Packet deserializer
    deserializer: Arc<PacketDeserializer>,
    
    /// Channel for sending packets
    send_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
    send_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<(Bytes, SocketAddr)>>>,
    
    /// Channel for receiving packets
    recv_tx: mpsc::UnboundedSender<PacketWrapper>,
    
    /// Local socket address
    local_addr: SocketAddr,
}

impl UdpTransport {
    /// Create a new UDP transport
    pub async fn new(config: TransportConfig) -> NetworkResult<Self> {
        // Create UDP socket
        let socket = UdpSocket::bind(config.bind_addr).await?;
        let local_addr = socket.local_addr()?;
        
        // Note: Socket buffer configuration not available on tokio::net::UdpSocket
        // This would require using socket2 crate for more advanced socket options
        
        debug!("UDP transport bound to {}", local_addr);
        
        let socket = Arc::new(socket);
        let deserializer = Arc::new(PacketDeserializer::new());
        
        // Create channels
        let (send_tx, send_rx) = mpsc::unbounded_channel();
        let (recv_tx, _) = mpsc::unbounded_channel();
        
        Ok(Self {
            socket,
            config,
            deserializer,
            send_tx,
            send_rx: Arc::new(tokio::sync::Mutex::new(send_rx)),
            recv_tx,
            local_addr,
        })
    }
    
    /// Get local socket address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    /// Get sender for outgoing packets
    pub fn get_sender(&self) -> mpsc::UnboundedSender<(Bytes, SocketAddr)> {
        self.send_tx.clone()
    }
    
    /// Get receiver for incoming packets
    pub fn get_receiver(&self) -> mpsc::UnboundedReceiver<PacketWrapper> {
        let (tx, rx) = mpsc::unbounded_channel();
        // Replace the internal receiver
        // Note: In a real implementation, you'd want a way to manage multiple receivers
        rx
    }
    
    /// Start the transport (begin processing packets)
    pub async fn start(&self) -> NetworkResult<()> {
        // Start background tasks
        self.start_packet_sender().await;
        self.start_packet_receiver().await;
        
        debug!("UDP transport started on {}", self.local_addr);
        Ok(())
    }
    
    /// Send a packet directly
    pub async fn send_packet(&self, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        if let Some(ref proxy) = self.config.proxy {
            self.send_via_proxy(data, dest, proxy).await
        } else {
            self.send_direct(data, dest).await
        }
    }
    
    /// Send packet directly via UDP
    async fn send_direct(&self, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        let bytes_sent = self.socket.send_to(&data, dest).await?;
        
        if bytes_sent != data.len() {
            warn!(
                "Partial send: {} bytes of {} to {}",
                bytes_sent,
                data.len(),
                dest
            );
        }
        
        debug!("Sent {} bytes to {}", bytes_sent, dest);
        Ok(())
    }
    
    /// Send packet via SOCKS5 proxy
    async fn send_via_proxy(&self, data: Bytes, dest: SocketAddr, proxy: &ProxyConfig) -> NetworkResult<()> {
        // For now, use the existing SOCKS5 UDP implementation
        // In a full implementation, we'd maintain persistent SOCKS5 connections
        // and integrate with the socks5_udp module properly
        
        // This is a simplified version - in practice you'd want to:
        // 1. Maintain a pool of SOCKS5 connections
        // 2. Use the existing Socks5UdpSocket implementation
        // 3. Handle connection failures and retries
        
        warn!("SOCKS5 proxy support not fully implemented yet");
        Err(NetworkError::Transport { 
            reason: "SOCKS5 proxy support not fully implemented".to_string() 
        })
    }
    
    /// Start packet sender task
    async fn start_packet_sender(&self) {
        let socket = Arc::clone(&self.socket);
        let send_rx = Arc::clone(&self.send_rx);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut rx = send_rx.lock().await;
            
            while let Some((data, dest)) = rx.recv().await {
                let result = if let Some(ref proxy) = config.proxy {
                    Self::send_via_proxy_static(&socket, data, dest, proxy).await
                } else {
                    Self::send_direct_static(&socket, data, dest).await
                };
                
                if let Err(e) = result {
                    error!("Failed to send packet to {}: {}", dest, e);
                }
            }
        });
    }
    
    /// Start packet receiver task
    async fn start_packet_receiver(&self) {
        let socket = Arc::clone(&self.socket);
        let deserializer = Arc::clone(&self.deserializer);
        let recv_tx = self.recv_tx.clone();
        let max_packet_size = self.config.max_packet_size;
        
        tokio::spawn(async move {
            let mut buffer = vec![0u8; max_packet_size];
            
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((len, src)) => {
                        debug!("Received {} bytes from {}", len, src);
                        
                        // Parse the packet
                        match deserializer.parse_raw(&buffer[..len]) {
                            Ok(packet_wrapper) => {
                                if let Err(_) = recv_tx.send(packet_wrapper) {
                                    // Receiver dropped - exit
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse packet from {}: {}", src, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("UDP receive error: {}", e);
                        // Consider whether to continue or break based on error type
                        if e.kind() == std::io::ErrorKind::ConnectionReset ||
                           e.kind() == std::io::ErrorKind::ConnectionAborted {
                            // Continue on connection resets (common in UDP)
                            continue;
                        }
                        break;
                    }
                }
            }
            
            debug!("UDP receiver task exiting");
        });
    }
    
    /// Static version of send_direct for use in spawned tasks
    async fn send_direct_static(socket: &UdpSocket, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        let bytes_sent = socket.send_to(&data, dest).await?;
        
        if bytes_sent != data.len() {
            warn!(
                "Partial send: {} bytes of {} to {}",
                bytes_sent,
                data.len(),
                dest
            );
        }
        
        Ok(())
    }
    
    /// Static version of send_via_proxy for use in spawned tasks  
    async fn send_via_proxy_static(
        _socket: &UdpSocket,
        _data: Bytes,
        _dest: SocketAddr,
        _proxy: &ProxyConfig,
    ) -> NetworkResult<()> {
        // Placeholder for SOCKS5 proxy support
        Err(NetworkError::Transport { 
            reason: "SOCKS5 proxy support not fully implemented".to_string() 
        })
    }
    
    /// Get transport statistics
    pub async fn get_stats(&self) -> TransportStats {
        // In a real implementation, you'd track these metrics
        TransportStats {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            send_errors: 0,
            receive_errors: 0,
        }
    }
}

/// Transport statistics
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub send_errors: u64,
    pub receive_errors: u64,
}