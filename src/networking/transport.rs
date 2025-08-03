//! UDP transport layer with SOCKS5 proxy support
//! 
//! Handles the low-level UDP socket operations, including optional SOCKS5 proxy
//! routing for environments that require proxy connections.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::PacketWrapper;
use crate::networking::serialization::PacketDeserializer;
use crate::networking::proxy::{ProxyConfig, Socks5UdpClient, HttpProxyClient};
use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, warn, info};

/// UDP transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Local bind address (127.0.0.1:0 for localhost)
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
            bind_addr: "0.0.0.0:0".parse().unwrap(), // Bind to all interfaces, not just localhost
            proxy: None,
            max_packet_size: 1500,
            recv_buffer_size: 64 * 1024,
            send_buffer_size: 64 * 1024,
        }
    }
}

/// UDP transport for Second Life protocol
pub struct UdpTransport {
    /// UDP socket (used for direct connections)
    socket: Arc<UdpSocket>,
    
    /// Transport configuration
    config: TransportConfig,
    
    /// SOCKS5 UDP client (when proxy is enabled)
    socks5_client: Arc<tokio::sync::RwLock<Option<Socks5UdpClient>>>,
    
    /// Transparent SOCKS5 proxy (for integrated WinHippoAutoProxy mode)
    transparent_proxy: Arc<tokio::sync::RwLock<Option<crate::networking::proxy::TransparentSocks5Proxy>>>,
    
    /// HTTP proxy client (when proxy is enabled)
    http_proxy_client: Arc<tokio::sync::RwLock<Option<HttpProxyClient>>>,
    
    /// Packet deserializer
    deserializer: Arc<PacketDeserializer>,
    
    /// Channel for sending packets
    send_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
    send_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<(Bytes, SocketAddr)>>>,
    
    /// Callback for receiving packets (called directly like homunculus)
    packet_callback: Arc<tokio::sync::RwLock<Option<Box<dyn Fn(PacketWrapper, SocketAddr) + Send + Sync>>>>,
    
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
        
        info!("UDP transport bound to {}", local_addr);
        
        let socket = Arc::new(socket);
        let deserializer = Arc::new(PacketDeserializer::new());
        
        // Create channels
        let (send_tx, send_rx) = mpsc::unbounded_channel();
        
        // Initialize proxy clients based on proxy mode
        let socks5_client = if let Some(ref proxy_config) = config.proxy {
            if proxy_config.requires_manual_socks5() || proxy_config.is_transparent_proxy() {
                // Create SOCKS5 client for both manual mode AND integrated transparent mode
                if let Some(socks5_addr) = proxy_config.socks5_addr {
                    let mode_desc = if proxy_config.requires_manual_socks5() { "manual" } else { "integrated transparent" };
                    info!("🔧 Initializing SOCKS5 client for {} mode: {}", mode_desc, socks5_addr);
                    let client = Socks5UdpClient::new(
                        socks5_addr,
                        proxy_config.username.clone(),
                        proxy_config.password.clone(),
                    );
                    Some(client)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        
        let http_proxy_client = if let Some(ref proxy_config) = config.proxy {
            if let Some(http_addr) = proxy_config.http_addr {
                match HttpProxyClient::new_with_ca_cert(
                    http_addr,
                    proxy_config.username.clone(),
                    proxy_config.password.clone(),
                    proxy_config.ca_cert_path.clone(),
                ) {
                    Ok(client) => Some(client),
                    Err(e) => {
                        warn!("Failed to create HTTP proxy client: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        
        // Initialize transparent proxy for integrated WinHippoAutoProxy mode
        let transparent_proxy = if let Some(ref proxy_config) = config.proxy {
            if proxy_config.is_transparent_proxy() {
                if let Some(socks5_addr) = proxy_config.socks5_addr {
                    info!("🔧 Initializing integrated transparent SOCKS5 proxy for {}", socks5_addr);
                    let mut proxy = crate::networking::proxy::TransparentSocks5Proxy::new(socks5_addr);
                    match proxy.initialize().await {
                        Ok(_) => Some(proxy),
                        Err(e) => {
                            warn!("Failed to initialize transparent proxy: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        
        Ok(Self {
            socket,
            config,
            socks5_client: Arc::new(tokio::sync::RwLock::new(socks5_client)),
            transparent_proxy: Arc::new(tokio::sync::RwLock::new(transparent_proxy)),
            http_proxy_client: Arc::new(tokio::sync::RwLock::new(http_proxy_client)),
            deserializer,
            send_tx,
            send_rx: Arc::new(tokio::sync::Mutex::new(send_rx)),
            packet_callback: Arc::new(tokio::sync::RwLock::new(None)),
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
    
    /// Set packet callback (called when packets are received, like homunculus)
    pub async fn set_packet_callback<F>(&self, callback: F) 
    where 
        F: Fn(PacketWrapper, SocketAddr) + Send + Sync + Clone + 'static
    {
        let mut cb = self.packet_callback.write().await;
        *cb = Some(Box::new(callback));
    }
    
    /// Start the transport (begin processing packets)
    pub async fn start(&self) -> NetworkResult<()> {
        // Handle proxy initialization based on mode
        if let Some(ref proxy_config) = self.config.proxy {
            match proxy_config.mode {
                crate::networking::proxy::ProxyMode::ManualSocks5 => {
                    // Connect to SOCKS5 proxy if using manual mode
                    if let Some(socks5_client) = self.socks5_client.read().await.as_ref() {
                        info!("🔗 Connecting to SOCKS5 proxy in manual mode...");
                        
                        // Retry connection with backoff
                        let mut attempts = 0;
                        let max_attempts = 3;
                        
                        loop {
                            attempts += 1;
                            match socks5_client.connect().await {
                                Ok(_) => {
                                    info!("✅ Successfully connected to SOCKS5 proxy after {} attempt(s)", attempts);
                                    break;
                                }
                                Err(e) => {
                                    if attempts >= max_attempts {
                                        error!("Failed to connect to SOCKS5 proxy after {} attempts: {}", attempts, e);
                                        return Err(e);
                                    }
                                    warn!("SOCKS5 connection attempt {} failed: {}, retrying...", attempts, e);
                                    tokio::time::sleep(std::time::Duration::from_millis(1000 * attempts as u64)).await;
                                }
                            }
                        }
                    } else {
                        return Err(NetworkError::Transport {
                            reason: "SOCKS5 client not configured for manual mode".to_string()
                        });
                    }
                }
                crate::networking::proxy::ProxyMode::WinHippoAutoProxy => {
                    info!("🔗 Connecting to Hippolyzer SOCKS5 proxy for integrated transparent mode...");
                    
                    // Even in transparent mode, we need to connect to SOCKS5 proxy
                    if let Some(socks5_client) = self.socks5_client.read().await.as_ref() {
                        // Retry connection with backoff
                        let mut attempts = 0;
                        let max_attempts = 3;
                        
                        loop {
                            attempts += 1;
                            match socks5_client.connect().await {
                                Ok(_) => {
                                    info!("✅ Successfully connected to Hippolyzer SOCKS5 proxy after {} attempt(s)", attempts);
                                    break;
                                }
                                Err(e) => {
                                    if attempts >= max_attempts {
                                        error!("Failed to connect to Hippolyzer SOCKS5 proxy after {} attempts: {}", attempts, e);
                                        return Err(e);
                                    }
                                    warn!("SOCKS5 connection attempt {} failed: {}, retrying...", attempts, e);
                                    tokio::time::sleep(std::time::Duration::from_millis(1000 * attempts as u64)).await;
                                }
                            }
                        }
                    } else {
                        return Err(NetworkError::Transport {
                            reason: "SOCKS5 client not configured for WinHippoAutoProxy mode".to_string()
                        });
                    }
                }
                crate::networking::proxy::ProxyMode::Direct => {
                    info!("🔗 Using direct connection (no proxy)");
                }
            }
        }
        
        // Start background tasks
        self.start_packet_sender().await;
        self.start_packet_receiver().await;
        
        info!("UDP transport started on {}", self.local_addr);
        Ok(())
    }
    
    /// Send a packet directly
    pub async fn send_packet(&self, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        info!("Transport sending {} bytes to {}", data.len(), dest);
        
        if let Some(ref proxy_config) = self.config.proxy {
            match proxy_config.mode {
                crate::networking::proxy::ProxyMode::ManualSocks5 => {
                    // Use manual SOCKS5 implementation
                    self.send_via_socks5(data, dest).await
                }
                crate::networking::proxy::ProxyMode::WinHippoAutoProxy => {
                    // Use SOCKS5 proxy for integrated transparent mode
                    self.send_via_socks5(data, dest).await
                }
                crate::networking::proxy::ProxyMode::Direct => {
                    // Direct connection, no proxy
                    self.send_direct(data, dest).await
                }
            }
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
        
        info!("Sent {} bytes to {}", bytes_sent, dest);
        
        // Debug: log first few bytes of sent data
        if data.len() > 0 {
            let hex_data: String = data[..std::cmp::min(data.len(), 32)]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            info!("   Sent Data (first 32 bytes): {}", hex_data);
        }
        Ok(())
    }
    
    /// Send packet via SOCKS5 proxy
    async fn send_via_socks5(&self, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        let socks5_guard = self.socks5_client.read().await;
        if let Some(socks5_client) = socks5_guard.as_ref() {
            socks5_client.send_to(&data, dest).await?;
            info!("Sent {} bytes via SOCKS5 proxy to {}", data.len(), dest);
            Ok(())
        } else {
            Err(NetworkError::Transport {
                reason: "SOCKS5 proxy not configured".to_string()
            })
        }
    }
    
    /// Send packet via integrated transparent SOCKS5 proxy
    async fn send_via_transparent_proxy(&self, data: Bytes, dest: SocketAddr) -> NetworkResult<()> {
        let transparent_guard = self.transparent_proxy.read().await;
        if let Some(transparent_proxy) = transparent_guard.as_ref() {
            transparent_proxy.send_transparent(&data, dest).await?;
            info!("📤 Sent {} bytes via integrated transparent SOCKS5 proxy to {}", data.len(), dest);
            Ok(())
        } else {
            Err(NetworkError::Transport {
                reason: "Transparent SOCKS5 proxy not configured".to_string()
            })
        }
    }
    
    /// Start packet sender task
    async fn start_packet_sender(&self) {
        let socket = Arc::clone(&self.socket);
        let send_rx = Arc::clone(&self.send_rx);
        let socks5_client = Arc::clone(&self.socks5_client);
        let proxy_config = self.config.proxy.clone();
        let has_proxy = proxy_config.is_some();
        
        tokio::spawn(async move {
            let mut rx = send_rx.lock().await;
            
            while let Some((data, dest)) = rx.recv().await {
                let result = if has_proxy {
                    // Check proxy mode to determine how to send
                    let proxy_cfg = proxy_config.as_ref().unwrap();
                    match proxy_cfg.mode {
                        crate::networking::proxy::ProxyMode::ManualSocks5 => {
                            // Use SOCKS5 client for manual mode
                            let connection_active = {
                                let socks5_guard = socks5_client.read().await;
                                if let Some(client) = socks5_guard.as_ref() {
                                    client.is_connected().await
                                } else {
                                    false
                                }
                            };
                            
                            if !connection_active {
                                error!("SOCKS5 connection lost, attempting to reconnect...");
                                let socks5_guard = socks5_client.read().await;
                                if let Some(client) = socks5_guard.as_ref() {
                                    match client.connect().await {
                                        Ok(_) => {
                                            info!("SOCKS5 reconnection successful");
                                            client.send_to(&data, dest).await
                                        }
                                        Err(e) => {
                                            error!("SOCKS5 reconnection failed: {}", e);
                                            Err(e)
                                        }
                                    }
                                } else {
                                    Err(NetworkError::Transport {
                                        reason: "SOCKS5 proxy not available in manual mode".to_string()
                                    })
                                }
                            } else {
                                let socks5_guard = socks5_client.read().await;
                                if let Some(client) = socks5_guard.as_ref() {
                                    client.send_to(&data, dest).await
                                } else {
                                    Err(NetworkError::Transport {
                                        reason: "SOCKS5 proxy not available in manual mode".to_string()
                                    })
                                }
                            }
                        }
                        crate::networking::proxy::ProxyMode::WinHippoAutoProxy => {
                            // Use SOCKS5 client for integrated transparent mode
                            // Skip connection checking for now - assume connection is active
                            // This prevents unnecessary reconnection attempts that cause Hippolyzer errors
                            let socks5_guard = socks5_client.read().await;
                            if let Some(client) = socks5_guard.as_ref() {
                                client.send_to(&data, dest).await
                            } else {
                                Err(NetworkError::Transport {
                                    reason: "SOCKS5 proxy not available in WinHippoAutoProxy mode".to_string()
                                })
                            }
                        }
                        crate::networking::proxy::ProxyMode::Direct => {
                            // Direct send
                            Self::send_direct_static(&socket, data, dest).await
                        }
                    }
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
        let socks5_client = Arc::clone(&self.socks5_client);
        let deserializer = Arc::clone(&self.deserializer);
        let packet_callback = Arc::clone(&self.packet_callback);
        let max_packet_size = self.config.max_packet_size;
        let proxy_config = self.config.proxy.clone();
        let has_proxy = proxy_config.is_some();
        
        tokio::spawn(async move {
            let mut buffer = vec![0u8; max_packet_size];
            
            loop {
                let (len, src) = if has_proxy {
                    let proxy_cfg = proxy_config.as_ref().unwrap();
                    match proxy_cfg.mode {
                        crate::networking::proxy::ProxyMode::ManualSocks5 => {
                            // Receive through SOCKS5 proxy in manual mode
                            let socks5_guard = socks5_client.read().await;
                            if let Some(client) = socks5_guard.as_ref() {
                                match client.recv_from(&mut buffer).await {
                                    Ok((len, src)) => (len, src),
                                    Err(e) => {
                                        error!("SOCKS5 receive error: {}", e);
                                        
                                        // Check if it's a connection error that might be recoverable
                                        if e.to_string().contains("connection") || e.to_string().contains("reset") {
                                            warn!("SOCKS5 connection may be lost, checking status...");
                                            if !client.is_connected().await {
                                                error!("SOCKS5 connection confirmed lost, receiver task exiting");
                                                break;
                                            }
                                        }
                                        continue;
                                    }
                                }
                            } else {
                                error!("SOCKS5 client not available in manual mode");
                                break;
                            }
                        }
                        crate::networking::proxy::ProxyMode::WinHippoAutoProxy => {
                            // Receive through SOCKS5 proxy in integrated transparent mode
                            let socks5_guard = socks5_client.read().await;
                            if let Some(client) = socks5_guard.as_ref() {
                                match client.recv_from(&mut buffer).await {
                                    Ok((len, src)) => (len, src),
                                    Err(e) => {
                                        error!("SOCKS5 receive error in WinHippoAutoProxy mode: {}", e);
                                        
                                        // Check if it's a connection error that might be recoverable
                                        if e.to_string().contains("connection") || e.to_string().contains("reset") {
                                            warn!("SOCKS5 connection may be lost in WinHippoAutoProxy mode, checking status...");
                                            if !client.is_connected().await {
                                                error!("SOCKS5 connection confirmed lost in WinHippoAutoProxy mode, receiver task exiting");
                                                break;
                                            }
                                        }
                                        continue;
                                    }
                                }
                            } else {
                                error!("SOCKS5 client not available in WinHippoAutoProxy mode");
                                break;
                            }
                        }
                        crate::networking::proxy::ProxyMode::Direct => {
                            // Direct UDP reception
                            match socket.recv_from(&mut buffer).await {
                                Ok((len, src)) => (len, src),
                                Err(e) => {
                                    error!("UDP receive error: {}", e);
                                    if e.kind() == std::io::ErrorKind::ConnectionReset ||
                                       e.kind() == std::io::ErrorKind::ConnectionAborted {
                                        continue;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    // Direct UDP reception
                    match socket.recv_from(&mut buffer).await {
                        Ok((len, src)) => (len, src),
                        Err(e) => {
                            error!("UDP receive error: {}", e);
                            if e.kind() == std::io::ErrorKind::ConnectionReset ||
                               e.kind() == std::io::ErrorKind::ConnectionAborted {
                                continue;
                            }
                            break;
                        }
                    }
                };
                
                info!("Received {} bytes from {}", len, src);
                
                // Debug: log first few bytes
                if len > 0 {
                    let hex_data: String = buffer[..std::cmp::min(len, 16)]
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    info!("First 16 bytes: {}", hex_data);
                }
                
                // Parse the packet
                match deserializer.parse_raw(&buffer[..len]) {
                    Ok(packet_wrapper) => {
                        info!("📥 NETWORKING RESPONSE: Successfully parsed packet from {}", src);
                        info!("   Packet ID: {}", packet_wrapper.packet_id);
                        info!("   Frequency: {:?}", packet_wrapper.frequency);
                        info!("   Reliable: {}", packet_wrapper.reliable);
                        info!("   Size: {} bytes", len);
                        
                        // Log packet name if available
                        let packet_name = match packet_wrapper.frequency {
                            crate::networking::packets::PacketFrequency::High => {
                                format!("High_{}", packet_wrapper.packet_id)
                            }
                            crate::networking::packets::PacketFrequency::Medium => {
                                format!("Medium_{}", packet_wrapper.packet_id)
                            }
                            crate::networking::packets::PacketFrequency::Low => {
                                format!("Low_{}", packet_wrapper.packet_id)
                            }
                            crate::networking::packets::PacketFrequency::Fixed => {
                                format!("Fixed_{}", packet_wrapper.packet_id)
                            }
                        };
                        info!("   Packet Type: {}", packet_name);
                        
                        // Enhanced hex dump for responses
                        if len > 0 {
                            let hex_data: String = buffer[..std::cmp::min(len, 32)]
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<_>>()
                                .join(" ");
                            info!("   Response Data (first 32 bytes): {}", hex_data);
                        }
                        
                        // Call the callback directly (like homunculus socket.receive)
                        let cb_guard = packet_callback.read().await;
                        if let Some(ref callback) = *cb_guard {
                            callback(packet_wrapper, src);
                        }
                        
                        info!("✅ NETWORKING RESPONSE: Packet processing completed successfully");
                    }
                    Err(e) => {
                        warn!("❌ NETWORKING RESPONSE ERROR: Failed to parse packet from {}", src);
                        warn!("   Error: {}", e);
                        warn!("   Raw data size: {} bytes", len);
                        if len > 0 {
                            let hex_data: String = buffer[..std::cmp::min(len, 16)]
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<_>>()
                                .join(" ");
                            warn!("   Raw data (first 16 bytes): {}", hex_data);
                        }
                    }
                }
            }
            
            info!("UDP receiver task exiting");
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
        
        info!("Sent {} bytes to {}", bytes_sent, dest);
        
        // Debug: log first few bytes of sent data
        if data.len() > 0 {
            let hex_data: String = data[..std::cmp::min(data.len(), 32)]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            info!("   Sent Data (first 32 bytes): {}", hex_data);
        }
        Ok(())
    }
    
    /// Get HTTP proxy client (if configured)
    pub async fn get_http_proxy_client(&self) -> Option<HttpProxyClient> {
        self.http_proxy_client.read().await.as_ref().cloned()
    }
    
    /// Check if SOCKS5 proxy is enabled and connected
    pub async fn is_socks5_connected(&self) -> bool {
        if let Some(client) = self.socks5_client.read().await.as_ref() {
            client.is_connected().await
        } else {
            false
        }
    }
    
    /// Check if HTTP proxy is enabled
    pub fn has_http_proxy(&self) -> bool {
        self.config.proxy.as_ref().map_or(false, |p| p.has_http())
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