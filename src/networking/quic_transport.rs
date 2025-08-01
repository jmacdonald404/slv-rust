//! QUIC transport layer implementing ADR-0002 networking protocol choice
//! 
//! This module implements the primary QUIC transport using quinn, with fallback
//! to raw UDP as specified in our networking documentation.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::PacketWrapper;
use crate::networking::serialization::PacketDeserializer;
use bytes::Bytes;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, warn, info};

/// QUIC transport configuration
#[derive(Debug, Clone)]
pub struct QuicTransportConfig {
    /// Local bind address (0.0.0.0:0 for any)
    pub bind_addr: SocketAddr,
    /// Connection timeout
    pub connect_timeout: std::time::Duration,
    /// Maximum packet size
    pub max_packet_size: usize,
    /// Enable fallback to UDP
    pub enable_udp_fallback: bool,
}

impl Default for QuicTransportConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:0".parse().unwrap(),
            connect_timeout: std::time::Duration::from_secs(30),
            max_packet_size: 1500,
            enable_udp_fallback: true,
        }
    }
}

/// QUIC transport for Second Life protocol following ADR-0002
pub struct QuicTransport {
    /// QUIC endpoint
    endpoint: Endpoint,
    
    /// Transport configuration
    config: QuicTransportConfig,
    
    /// Active QUIC connection
    connection: Arc<tokio::sync::RwLock<Option<Connection>>>,
    
    /// Packet deserializer
    deserializer: Arc<PacketDeserializer>,
    
    /// Channel for sending packets
    send_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
    send_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<(Bytes, SocketAddr)>>>,
    
    /// Callback for receiving packets
    packet_callback: Arc<tokio::sync::RwLock<Option<Box<dyn Fn(PacketWrapper, SocketAddr) + Send + Sync>>>>,
    
    /// Local socket address
    local_addr: SocketAddr,
    
    /// UDP fallback transport
    udp_fallback: Option<Arc<crate::networking::transport::UdpTransport>>,
}

impl QuicTransport {
    /// Create a new QUIC transport following ADR-0002
    pub async fn new(config: QuicTransportConfig) -> NetworkResult<Self> {
        info!("🔐 Initializing QUIC transport following ADR-0002 networking protocol choice");
        
        // Configure QUIC client with TLS 1.3 security
        let client_config = ClientConfig::with_platform_verifier();
        
        // Create QUIC endpoint
        let endpoint = Endpoint::client(config.bind_addr)
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to create QUIC endpoint: {}", e) })?;
        
        let local_addr = endpoint.local_addr()
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to get local address: {}", e) })?;
        
        info!("🔐 QUIC transport bound to {} with TLS 1.3 encryption", local_addr);
        
        let deserializer = Arc::new(PacketDeserializer::new());
        
        // Create channels
        let (send_tx, send_rx) = mpsc::unbounded_channel();
        
        // Initialize UDP fallback if enabled
        let udp_fallback = if config.enable_udp_fallback {
            info!("🔄 UDP fallback enabled for compatibility with legacy environments");
            let udp_config = crate::networking::transport::TransportConfig {
                bind_addr: config.bind_addr,
                proxy: None,
                max_packet_size: config.max_packet_size,
                recv_buffer_size: 64 * 1024,
                send_buffer_size: 64 * 1024,
            };
            match crate::networking::transport::UdpTransport::new(udp_config).await {
                Ok(transport) => Some(Arc::new(transport)),
                Err(e) => {
                    warn!("Failed to initialize UDP fallback: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        Ok(Self {
            endpoint,
            config,
            connection: Arc::new(tokio::sync::RwLock::new(None)),
            deserializer,
            send_tx,
            send_rx: Arc::new(tokio::sync::Mutex::new(send_rx)),
            packet_callback: Arc::new(tokio::sync::RwLock::new(None)),
            local_addr,
            udp_fallback,
        })
    }
    
    /// Connect to a QUIC server with UDP fallback
    pub async fn connect(&self, server_addr: SocketAddr, server_name: &str) -> NetworkResult<()> {
        info!("🔐 Attempting QUIC connection to {} ({})", server_addr, server_name);
        
        // Try QUIC connection first
        match tokio::time::timeout(
            self.config.connect_timeout,
            async {
                let connecting = self.endpoint.connect(server_addr, server_name)
                    .map_err(|e| NetworkError::Transport { reason: format!("Failed to connect: {}", e) })?;
                connecting.await
                    .map_err(|e| NetworkError::Transport { reason: format!("Connection failed: {}", e) })
            }
        ).await {
            Ok(Ok(connection)) => {
                info!("✅ QUIC connection established with TLS 1.3 security and reliability");
                let mut conn_guard = self.connection.write().await;
                *conn_guard = Some(connection);
                return Ok(());
            }
            Ok(Err(e)) => {
                warn!("QUIC connection failed: {}", e);
            }
            Err(_) => {
                warn!("QUIC connection timed out after {}s", self.config.connect_timeout.as_secs());
            }
        }
        
        // Fallback to UDP if enabled
        if let Some(ref udp_transport) = self.udp_fallback {
            warn!("🔄 Falling back to raw UDP transport for compatibility");
            udp_transport.start().await?;
            return Ok(());
        }
        
        Err(NetworkError::SimulatorConnectionFailed {
            reason: "QUIC connection failed and UDP fallback not available".to_string()
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
    
    /// Set packet callback
    pub async fn set_packet_callback<F>(&self, callback: F) 
    where 
        F: Fn(PacketWrapper, SocketAddr) + Send + Sync + Clone + 'static
    {
        let mut cb = self.packet_callback.write().await;
        *cb = Some(Box::new(callback.clone()));
        
        // Also set callback on UDP fallback if available
        if let Some(ref udp_transport) = self.udp_fallback {
            udp_transport.set_packet_callback(callback).await;
        }
    }
    
    /// Start the transport
    pub async fn start(&self) -> NetworkResult<()> {
        self.start_packet_sender().await;
        self.start_packet_receiver().await;
        
        info!("🔐 QUIC transport started with modern security and congestion control");
        Ok(())
    }
    
    /// Send a packet (QUIC or UDP fallback)
    pub async fn send_packet(&self, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        // Try QUIC first
        if let Some(connection) = self.connection.read().await.as_ref() {
            return self.send_via_quic(connection, data).await;
        }
        
        // Fallback to UDP if available
        if let Some(ref udp_transport) = self.udp_fallback {
            return udp_transport.send_packet(data, dest).await;
        }
        
        Err(NetworkError::ConnectionLost { address: dest })
    }
    
    /// Send packet via QUIC stream
    async fn send_via_quic(&self, connection: &Connection, data: Bytes) -> NetworkResult<()> {
        match connection.open_uni().await {
            Ok(mut send_stream) => {
                send_stream.write_all(&data).await
                    .map_err(|e| NetworkError::Transport { reason: e.to_string() })?;
                send_stream.finish()
                    .map_err(|e| NetworkError::Transport { reason: e.to_string() })?;
                
                info!("🔐 Sent {} bytes via QUIC with built-in reliability", data.len());
                Ok(())
            }
            Err(e) => Err(NetworkError::Transport { reason: e.to_string() })
        }
    }
    
    /// Start packet sender task
    async fn start_packet_sender(&self) {
        let send_rx = Arc::clone(&self.send_rx);
        let connection = Arc::clone(&self.connection);
        let udp_fallback = self.udp_fallback.clone();
        
        tokio::spawn(async move {
            let mut rx = send_rx.lock().await;
            
            while let Some((data, dest)) = rx.recv().await {
                let result = {
                    let conn_guard = connection.read().await;
                    if let Some(conn) = conn_guard.as_ref() {
                        Self::send_via_quic_static(conn, data.clone()).await
                    } else if let Some(ref udp) = udp_fallback {
                        udp.send_packet(data, dest).await
                    } else {
                        Err(NetworkError::ConnectionLost { address: dest })
                    }
                };
                
                if let Err(e) = result {
                    error!("Failed to send packet to {}: {}", dest, e);
                }
            }
        });
    }
    
    /// Start packet receiver task
    async fn start_packet_receiver(&self) {
        let connection = Arc::clone(&self.connection);
        let deserializer = Arc::clone(&self.deserializer);
        let packet_callback = Arc::clone(&self.packet_callback);
        
        tokio::spawn(async move {
            loop {
                let conn_guard = connection.read().await;
                if let Some(connection) = conn_guard.as_ref() {
                    // Accept incoming QUIC streams
                    match connection.accept_uni().await {
                        Ok(recv_stream) => {
                            let deserializer = Arc::clone(&deserializer);
                            let packet_callback = Arc::clone(&packet_callback);
                            let remote_addr = connection.remote_address();
                            
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_quic_stream(recv_stream, deserializer, packet_callback, remote_addr).await {
                                    warn!("Error handling QUIC stream: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            debug!("QUIC stream accept failed: {}", e);
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                } else {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        });
    }
    
    /// Handle incoming QUIC stream
    async fn handle_quic_stream(
        mut recv_stream: RecvStream,
        deserializer: Arc<PacketDeserializer>,
        packet_callback: Arc<tokio::sync::RwLock<Option<Box<dyn Fn(PacketWrapper, SocketAddr) + Send + Sync>>>>,
        remote_addr: SocketAddr,
    ) -> NetworkResult<()> {
        let buffer = recv_stream.read_to_end(1500).await
            .map_err(|e| NetworkError::Transport { reason: e.to_string() })?;
        
        info!("🔐 Received {} bytes via QUIC from {}", buffer.len(), remote_addr);
        
        // Parse the packet
        match deserializer.parse_raw(&buffer) {
            Ok(packet_wrapper) => {
                info!("Successfully parsed QUIC packet: id={}, reliable via QUIC built-in", packet_wrapper.packet_id);
                
                // Call the callback
                let cb_guard = packet_callback.read().await;
                if let Some(ref callback) = *cb_guard {
                    callback(packet_wrapper, remote_addr);
                }
            }
            Err(e) => {
                warn!("Failed to parse QUIC packet from {}: {}", remote_addr, e);
            }
        }
        
        Ok(())
    }
    
    /// Static version of send_via_quic for use in spawned tasks
    async fn send_via_quic_static(connection: &Connection, data: Bytes) -> NetworkResult<()> {
        match connection.open_uni().await {
            Ok(mut send_stream) => {
                send_stream.write_all(&data).await
                    .map_err(|e| NetworkError::Transport { reason: e.to_string() })?;
                send_stream.finish()
                    .map_err(|e| NetworkError::Transport { reason: e.to_string() })?;
                Ok(())
            }
            Err(e) => Err(NetworkError::Transport { reason: e.to_string() })
        }
    }
    
    /// Check if QUIC connection is active
    pub async fn is_quic_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }
    
    /// Check if using UDP fallback
    pub fn has_udp_fallback(&self) -> bool {
        self.udp_fallback.is_some()
    }
}