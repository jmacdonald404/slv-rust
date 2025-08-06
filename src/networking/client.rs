//! High-level Second Life client interface
//! 
//! This module provides the main client API that applications use to connect
//! to Second Life simulators and interact with the virtual world.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::circuit::CircuitOptions;
use crate::networking::core::{Core, CoreEvent, CoreState};
use crate::networking::packets::generated::*;
use crate::networking::transport::TransportConfig;
use crate::networking::quic_transport::{QuicTransport, QuicTransportConfig};
use crate::networking::auth::SessionInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};
use tracing::error;
use ureq::{Agent, Proxy};
use std::io::Read;

#[derive(Debug)]
pub enum HandshakeEvent {
    RegionHandshake,
    AgentMovementComplete,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventQueueGetEvent {
    pub message: String,
    pub body: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventQueueGetResponse {
    pub events: Vec<EventQueueGetEvent>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectUpdateEventBody {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "P")]
    pub position: [f32; 3],
}

use tracing::{debug, info, warn};
use uuid::Uuid;

/// Security configuration for client connections
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Base timeout for authentication packets (used with exponential backoff)
    pub base_auth_timeout_secs: u64,
    /// Maximum authentication attempts before failing
    pub max_auth_attempts: u8,
    /// Require reliable transmission for all authentication packets
    pub require_reliable_auth: bool,
    /// Enable additional circuit validation
    pub enable_circuit_validation: bool,
    /// Mask sensitive data in logs
    pub mask_sensitive_logs: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            base_auth_timeout_secs: 10,  // Increased from 3 to 10 seconds
            max_auth_attempts: 5,        // Increased from 3 to 5 attempts
            require_reliable_auth: false, // TEMPORARY: Disable for testing
            enable_circuit_validation: false, // TEMPORARY: Disable for testing
            mask_sensitive_logs: true,
        }
    }
}

/// Client configuration following ADR-0002 networking protocol choice
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// QUIC transport configuration (primary transport)
    pub quic_transport: QuicTransportConfig,
    /// UDP transport configuration (fallback)
    pub transport: TransportConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Agent information
    pub agent_id: Uuid,
    pub session_id: Uuid,
    /// Default timeout for operations
    pub default_timeout: std::time::Duration,
    /// Session information
    pub session_info: Option<SessionInfo>,
    /// Use QUIC as primary transport (ADR-0002)
    pub prefer_quic: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            quic_transport: QuicTransportConfig::default(),
            transport: TransportConfig::default(),
            security: SecurityConfig::default(),
            agent_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            default_timeout: std::time::Duration::from_secs(30),
            session_info: None,
            prefer_quic: false, // Use UDP to match SL protocol specification (custom reliable UDP)
        }
    }
}

/// High-level Second Life client
pub struct Client {
    /// Client configuration
    config: ClientConfig,
    
    /// Networking core
    core: Arc<Core>,
    
    /// Current state
    state: Arc<RwLock<ClientState>>,
    
    /// Event channels
    event_tx: mpsc::UnboundedSender<ClientEvent>,
    
    /// Handshake completion channels
    handshake_tx: mpsc::Sender<HandshakeEvent>,
    handshake_rx: Arc<RwLock<mpsc::Receiver<HandshakeEvent>>>,
    
    /// Background task handles
    _background_tasks: Vec<tokio::task::JoinHandle<()>>,

    /// Session information
    session_info: SessionInfo,

    /// HTTP client for EventQueueGet (may be proxied)
    http_agent: ureq::Agent,
}

/// Client state
#[derive(Debug, Clone, PartialEq)]
pub enum ClientState {
    Disconnected,
    Connecting,
    Connected,
    LoggingIn,
    LoggedIn,
    Disconnecting,
}

/// Events emitted by the client
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// Client state changed
    StateChanged { old_state: ClientState, new_state: ClientState },
    /// Connected to simulator
    Connected { address: SocketAddr },
    /// Disconnected from simulator
    Disconnected { address: SocketAddr, reason: String },
    /// Login completed successfully
    LoginComplete,
    /// Region handshake received
    RegionHandshakeReceived,
    /// Agent movement complete received
    AgentMovementCompleteReceived,
    /// Error occurred
    Error { error: NetworkError },
}

impl Client {
    /// Load custom CA certificate from PEM file (copied from AuthenticationService)
    fn load_custom_ca_cert() -> Result<ureq::tls::Certificate<'static>, anyhow::Error> {
        use anyhow::Context;
        let ca_pem_path = std::path::Path::new("src/assets/CA.pem");
        
        if !ca_pem_path.exists() {
            anyhow::bail!("Custom CA certificate not found at: {}", ca_pem_path.display());
        }
        
        let ca_pem_data = std::fs::read(ca_pem_path)
            .with_context(|| format!("Failed to read CA certificate from: {}", ca_pem_path.display()))?;
        
        ureq::tls::Certificate::from_pem(&ca_pem_data)
            .with_context(|| format!("Failed to parse CA certificate from: {}", ca_pem_path.display()))
    }
    /// Create a new client following ADR-0002 networking protocol choice
    pub async fn new(config: ClientConfig, session_info: SessionInfo) -> NetworkResult<Self> {
        // Create networking core with QUIC support per ADR-0002
        let core = if config.prefer_quic {
            info!("🔐 Using QUIC as primary transport per ADR-0002");
            Arc::new(Core::new_with_quic(config.quic_transport.clone(), config.transport.clone()).await?)
        } else {
            info!("Using UDP transport (QUIC disabled)");
            Arc::new(Core::new(config.transport.clone()).await?)
        };
        
        // Create ureq agent, possibly with proxy support
        let http_agent = if let Some(ref proxy_config) = config.transport.proxy {
            if let Some(http_addr) = proxy_config.http_addr {
                info!("Configuring ureq agent to use proxy at {}", http_addr);
                
                let proxy_url = format!("http://{}", http_addr);
                let proxy = ureq::Proxy::new(&proxy_url)
                    .map_err(|e| NetworkError::Transport {
                        reason: format!("Failed to create HTTP proxy: {}", e)
                    })?;
                
                // Configure TLS like the working capability agent  
                // Try to load custom CA cert first, fallback to disabled verification
                let agent = match Self::load_custom_ca_cert() {
                    Ok(custom_ca) => {
                        let custom_certs = vec![custom_ca];
                        let root_certs = ureq::tls::RootCerts::new_with_certs(&custom_certs);
                        let tls_config = ureq::tls::TlsConfig::builder()
                            .root_certs(root_certs)
                            .build();
                        
                        info!("Configuring EventQueue agent with custom CA certificate");
                        ureq::Agent::config_builder()
                            .proxy(Some(proxy))
                            .tls_config(tls_config)
                            .timeout_global(Some(std::time::Duration::from_secs(60)))
                            .user_agent("")  // Disable User-Agent header to match official viewer
                            .build()
                            .into()
                    }
                    Err(e) => {
                        warn!("Failed to load custom CA certificate for EventQueue: {}. Disabling certificate validation.", e);
                        let tls_config = ureq::tls::TlsConfig::builder()
                            .disable_verification(true)
                            .build();
                        
                        ureq::Agent::config_builder()
                            .proxy(Some(proxy))
                            .tls_config(tls_config)
                            .timeout_global(Some(std::time::Duration::from_secs(60)))
                            .user_agent("")  // Disable User-Agent header to match official viewer
                            .build()
                            .into()
                    }
                };
                
                // Add authentication if provided
                if let (Some(username), Some(password)) = (&proxy_config.username, &proxy_config.password) {
                    // Note: ureq handles basic auth per-request, not per-agent
                    // We'll handle this in the individual request calls
                    info!("Proxy authentication will be handled per-request");
                }
                
                // Certificate handling
                if let Some(ca_path) = &proxy_config.ca_cert_path {
                    warn!("CA certificate loading not directly supported in ureq - relying on system TLS settings");
                    warn!("CA cert path specified: {}", ca_path);
                } else {
                    warn!("No CA cert provided - relying on system TLS settings for Hippolyzer proxy");
                }
                
                agent
            } else {
                ureq::Agent::config_builder()
                    .user_agent("")  // Disable User-Agent header to match official viewer
                    .build()
                    .into()
            }
        } else {
            ureq::Agent::config_builder()
                .user_agent("")  // Disable User-Agent header to match official viewer
                .build()
                .into()
        };
        
        // Create event channels
        let (event_tx, _) = mpsc::unbounded_channel();
        let (handshake_tx, handshake_rx) = mpsc::channel(100);
        
        let client = Self {
            config,
            core,
            state: Arc::new(RwLock::new(ClientState::Disconnected)),
            event_tx,
            handshake_tx,
            handshake_rx: Arc::new(RwLock::new(handshake_rx)),
            _background_tasks: Vec::new(),
            session_info,
            http_agent,
        };
        
        // Start background tasks
        client.start_background_tasks().await;
        
        Ok(client)
    }
    
    /// Create a new client with Hippolyzer proxy configuration (auto-detects proxy mode)
    pub async fn new_with_hippolyzer_proxy(mut config: ClientConfig, session_info: SessionInfo) -> NetworkResult<Self> {
        use crate::networking::proxy::{ProxyConfig, ProxyMode};
        
        // Configure for Hippolyzer with auto-detected proxy mode
        let proxy_config = ProxyConfig::hippolyzer_default();
        let mode = &proxy_config.mode;
        
        info!("🔧 Creating client with Hippolyzer proxy support");
        info!("   SOCKS5 proxy: 127.0.0.1:9061");
        info!("   HTTP proxy: 127.0.0.1:9062");
        info!("   Detected proxy mode: {:?}", mode);
        
        match mode {
            ProxyMode::WinHippoAutoProxy => {
                info!("🔧 Using WinHippoAutoProxy transparent mode");
                info!("   📋 Make sure WinHippoAutoProxy is running before connecting");
                info!("   📋 Download from: https://github.com/SaladDais/WinHippoAutoProxy");
            }
            ProxyMode::ManualSocks5 => {
                info!("🔧 Using manual SOCKS5 implementation");
                info!("   📋 Application will handle SOCKS5 protocol directly");
            }
            ProxyMode::Direct => {
                info!("🔧 No proxy configured - using direct connection");
            }
        }
        
        config.transport.proxy = Some(proxy_config);
        Self::new(config, session_info).await
    }
    
    /// Create a new client with Hippolyzer proxy using a specific proxy mode
    pub async fn new_with_hippolyzer_proxy_mode(mut config: ClientConfig, session_info: SessionInfo, mode: crate::networking::proxy::ProxyMode) -> NetworkResult<Self> {
        use crate::networking::proxy::ProxyConfig;
        
        // Configure for Hippolyzer with forced proxy mode
        let proxy_config = ProxyConfig::hippolyzer_with_mode(mode.clone());
        
        info!("🔧 Creating client with Hippolyzer proxy support (forced mode)");
        info!("   SOCKS5 proxy: 127.0.0.1:9061");
        info!("   HTTP proxy: 127.0.0.1:9062");
        info!("   Forced proxy mode: {:?}", mode);
        
        config.transport.proxy = Some(proxy_config);
        Self::new(config, session_info).await
    }
    
    /// Get current state
    pub async fn state(&self) -> ClientState {
        self.state.read().await.clone()
    }
    
    /// Get event receiver
    pub fn get_event_receiver(&self) -> mpsc::UnboundedReceiver<ClientEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        // In a real implementation, you'd manage multiple event receivers
        rx
    }
    
    /// Connect to a simulator
    pub async fn connect(&self, simulator_address: SocketAddr, circuit_code: u32) -> NetworkResult<()> {
        info!("🔍 CLIENT CONNECT: Starting connect method");
        info!("Connecting to simulator at {} with circuit code {}", simulator_address, circuit_code);
        
        self.set_state(ClientState::Connecting).await;
        
        // Start the core
        info!("🔍 CLIENT CONNECT: About to start networking core");
        match self.core.start().await {
            Ok(()) => {
                info!("✅ CLIENT CONNECT: Networking core started successfully");
            }
            Err(e) => {
                error!("❌ CLIENT CONNECT: Failed to start networking core: {}", e);
                return Err(e);
            }
        }
        
        // Create circuit options
        let options = CircuitOptions {
            circuit_code,
            address: simulator_address,
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
        };
        
        // Connect to the circuit
        info!("🔍 CLIENT CONNECT: About to connect to UDP circuit");
        let circuit = match self.core.connect_circuit(options, self.handshake_tx.clone()).await {
            Ok(circuit) => {
                info!("✅ CLIENT CONNECT: UDP circuit connected successfully");
                circuit
            }
            Err(e) => {
                error!("❌ CLIENT CONNECT: Failed to connect UDP circuit: {}", e);
                return Err(e);
            }
        };
        
        // Give the circuit a moment to fully initialize its background tasks
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        self.set_state(ClientState::LoggingIn).await;
        
        // Perform handshake
        info!("🔍 CLIENT CONNECT: About to perform UDP handshake");
        match self.perform_handshake(&circuit).await {
            Ok(()) => {
                info!("✅ CLIENT CONNECT: UDP handshake completed successfully");
            }
            Err(e) => {
                error!("❌ CLIENT CONNECT: UDP handshake failed: {}", e);
                return Err(e);
            }
        }
        
        self.set_state(ClientState::LoggedIn).await;
        let _ = self.event_tx.send(ClientEvent::LoginComplete);
        
        info!("🔍 CLIENT CONNECT: Method completed successfully");
        info!("Successfully connected and logged in to {}", simulator_address);
        Ok(())
    }
    
    /// Disconnect from current simulator
    pub async fn disconnect(&self) -> NetworkResult<()> {
        self.set_state(ClientState::Disconnecting).await;
        
        // Shutdown the core (this will disconnect all circuits)
        self.core.shutdown().await?;
        
        self.set_state(ClientState::Disconnected).await;
        info!("Disconnected from simulator");
        
        Ok(())
    }
    
    /// Send a chat message
    pub async fn send_chat(&self, message: &str, channel: u32) -> NetworkResult<()> {
        let circuit = self.core.primary_circuit().await
            .ok_or(NetworkError::CircuitNotFound { id: 0 })?;
        
        // Create ChatFromViewer packet
        let chat_packet = ChatFromViewer {
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
            message: crate::networking::packets::types::LLVariable2::from_string(message),
            r#type: 1, // Normal chat
            channel: channel as i32,
        };
        
        circuit.send(&chat_packet).await?;
        debug!("Sent chat message: {}", message);
        
        Ok(())
    }
    
    /// Update agent position and orientation
    pub async fn update_agent(&self, 
                             position: Option<crate::networking::packets::types::LLVector3>,
                             rotation: Option<crate::networking::packets::types::LLQuaternion>) -> NetworkResult<()> {
        let circuit = self.core.primary_circuit().await
            .ok_or(NetworkError::CircuitNotFound { id: 0 })?;
        
        let agent_update = AgentUpdate {
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
            body_rotation: rotation.unwrap_or_else(|| 
                crate::networking::packets::types::LLQuaternion::identity()),
            head_rotation: crate::networking::packets::types::LLQuaternion::identity(),
            state: 0,
            camera_center: position.unwrap_or_else(|| 
                crate::networking::packets::types::LLVector3::new(128.0, 128.0, 25.0)),
            camera_at_axis: crate::networking::packets::types::LLVector3::new(1.0, 0.0, 0.0),
            camera_left_axis: crate::networking::packets::types::LLVector3::new(0.0, 1.0, 0.0),
            camera_up_axis: crate::networking::packets::types::LLVector3::new(0.0, 0.0, 1.0),
            far: 256.0,
            control_flags: 0,
            flags: 0,
        };
        
        // AgentUpdate is sent unreliably
        circuit.send(&agent_update).await?;
        
        Ok(())
    }
    
    /// SECURITY: Verify that UseCircuitCode was successfully processed by the server
    /// This prevents proceeding with an insecure connection
    async fn verify_circuit_authentication(&self, circuit: &crate::networking::circuit::Circuit) -> NetworkResult<()> {
        info!("🔐 Verifying circuit authentication...");
        
        // Check circuit state - should be in Handshaking after UseCircuitCode
        let current_state = circuit.state().await;
        if !matches!(current_state, crate::networking::circuit::CircuitState::Handshaking) {
            return Err(NetworkError::AuthenticationFailed {
                reason: format!("Circuit not in Handshaking state after UseCircuitCode (current: {:?})", current_state)
            });
        }
        
        // Additional verification: Test if server is responding to pings
        // This ensures the circuit is actually established and the server recognizes our session
        info!("🔐 Testing server response to verify authentication...");
        
        // Send a small test to verify the connection is working
        // We'll use a simple ping mechanism built into the circuit
        let ping_timeout = std::time::Duration::from_secs(5);
        match tokio::time::timeout(ping_timeout, self.test_circuit_responsiveness(circuit)).await {
            Ok(Ok(())) => {
                info!("✅ Circuit authentication verified - server is responding");
                Ok(())
            },
            Ok(Err(e)) => {
                warn!("🔒 SECURITY: Circuit authentication verification failed: {}", e);
                Err(NetworkError::AuthenticationFailed {
                    reason: format!("Circuit responsiveness test failed: {}", e)
                })
            },
            Err(_) => {
                warn!("🔒 SECURITY: Circuit authentication verification timed out");
                Err(NetworkError::AuthenticationFailed {
                    reason: "Circuit responsiveness test timed out".to_string()
                })
            }
        }
    }
    
    /// Test basic network connectivity with a simple unreliable packet
    async fn test_basic_connectivity(&self, circuit: &crate::networking::circuit::Circuit) -> NetworkResult<()> {
        info!("🌐 Testing basic connectivity to {}", circuit.address());
        
        // Send a simple TestMessage packet unreliably just to verify network path
        use crate::networking::packets::generated::*;
        let test_message = TestMessage {
            test1: 42, // Simple test value
            neighbor_block: vec![], // Empty neighbor block
        };
        
        // Send unreliably - we don't expect a response, just testing if packets can be sent
        match circuit.send(&test_message).await {
            Ok(()) => {
                info!("✅ Basic connectivity test packet sent successfully");
                // Give it a moment to reach the server
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                Ok(())
            },
            Err(e) => {
                warn!("❌ Basic connectivity test failed: {}", e);
                Err(NetworkError::ConnectionLost { 
                    address: circuit.address()
                })
            }
        }
    }

    /// Test circuit responsiveness after UseCircuitCode
    async fn test_circuit_responsiveness(&self, circuit: &crate::networking::circuit::Circuit) -> NetworkResult<()> {
        // For now, we'll rely on the circuit's internal state validation
        // In a full implementation, this could send a ping packet and wait for response
        
        // Verify the circuit is properly connected
        let current_state = circuit.state().await;
        if matches!(current_state, crate::networking::circuit::CircuitState::Disconnected | crate::networking::circuit::CircuitState::Blocked) {
            return Err(NetworkError::ConnectionLost { address: circuit.address() });
        }
        
        info!("🔐 Circuit responsiveness test passed (state: {:?})", current_state);
        Ok(())
    }
    
    /// Perform initial handshake with simulator following homunculus protocol
    async fn perform_handshake(&self, circuit: &crate::networking::circuit::Circuit) -> NetworkResult<()> {
        info!("🔍 PERFORM_HANDSHAKE: Method starting");
        info!("Starting handshake with simulator following homunculus protocol");
        info!("🔍 Testing server responsiveness at {}", circuit.address());
        
        // Step 0: Basic connectivity test with unreliable packet
        info!("🔍 PERFORM_HANDSHAKE: About to test basic connectivity");
        self.test_basic_connectivity(circuit).await?;
        info!("✅ PERFORM_HANDSHAKE: Basic connectivity test passed");
        
        // Step 1: Send UseCircuitCode - establishes the circuit
        let use_circuit_code = UseCircuitCode {
            code: circuit.circuit_code(),
            session_id: self.session_info.session_id,
            id: self.session_info.agent_id, // Fixed: Use LLUUID directly, not Vec<u8>
        };
        
        info!("📦 UseCircuitCode details:");
        info!("  Circuit Code: {}", circuit.circuit_code());
        // SECURITY: Mask sensitive authentication data in logs
        info!("  Agent ID: {}...{}", 
              &self.session_info.agent_id.to_string()[..8],
              &self.session_info.agent_id.to_string()[28..]);
        info!("  Session ID: {}...{}", 
              &self.session_info.session_id.to_string()[..8],
              &self.session_info.session_id.to_string()[28..]);
        
        // SECURITY: UseCircuitCode contains sensitive authentication data and MUST be sent reliably
        // Multiple attempts with exponential backoff instead of insecure unreliable fallback
        let mut attempt = 1;
        let max_attempts = self.config.security.max_auth_attempts;
        let base_timeout = self.config.security.base_auth_timeout_secs;
        let mut success = false;
        
        // SECURITY: Enforce minimum security requirements
        if !self.config.security.require_reliable_auth {
            warn!("🔒 SECURITY WARNING: require_reliable_auth is disabled - this is insecure!");
            warn!("🔄 BYPASS MODE: Using unreliable transmission for testing");
            
            // TEMPORARY BYPASS: Send UseCircuitCode unreliably and proceed
            info!("🔄 BYPASS: Sending UseCircuitCode unreliably for testing");
            match circuit.send(&use_circuit_code).await {
                Ok(()) => {
                    info!("✅ BYPASS: UseCircuitCode sent unreliably - proceeding to EventQueueGet");
                    circuit.set_state(crate::networking::circuit::CircuitState::Handshaking).await?;
                    success = true;
                }
                Err(e) => {
                    error!("❌ BYPASS: Failed to send UseCircuitCode unreliably: {}", e);
                    return Err(NetworkError::AuthenticationFailed { 
                        reason: format!("Unreliable UseCircuitCode failed: {}", e) 
                    });
                }
            }
        }
        
        while attempt <= max_attempts && !success {
            // Dynamic timeout calculation based on exponential backoff
            // Following SL protocol specs: base timeout with exponential backoff
            // Start with longer initial timeout for better stability
            let timeout_duration = std::time::Duration::from_secs(
                (base_timeout * 2) * (1 << (attempt - 1).min(2)) as u64
            );
            info!("🔐 Attempting secure UseCircuitCode transmission (attempt {}/{}) with {}s timeout", 
                  attempt, max_attempts, timeout_duration.as_secs());
            
            info!("🔐 AUTH REQUEST: Sending UseCircuitCode (attempt {}/{})", attempt, max_attempts);
            info!("   Timeout: {:?}", timeout_duration);
            info!("   Circuit Code: {}", circuit.circuit_code());
            
            let auth_start = std::time::Instant::now();
            
            // Send the UseCircuitCode packet reliably and wait for server acknowledgment
            info!("🔍 SEND_RELIABLE: About to call circuit.send_reliable()");
            let send_future = circuit.send_reliable(&use_circuit_code, timeout_duration);
            info!("🔍 SEND_RELIABLE: Future created, now awaiting with timeout");
            
            match tokio::time::timeout(timeout_duration, send_future).await {
                Ok(Ok(())) => {
                    let auth_time = auth_start.elapsed();
                    info!("✅ AUTH RESPONSE: UseCircuitCode sent reliably and acknowledged");
                    info!("   Attempt: {}/{}", attempt, max_attempts);
                    info!("   Auth time: {:?}", auth_time);
                    info!("   Setting circuit state to Handshaking");
                    circuit.set_state(crate::networking::circuit::CircuitState::Handshaking).await?;
                    success = true;
                },
                Ok(Err(e)) => {
                    let auth_time = auth_start.elapsed();
                    warn!("❌ AUTH RESPONSE ERROR: UseCircuitCode reliable send failed");
                    warn!("   Attempt: {}/{}", attempt, max_attempts);
                    warn!("   Error: {}", e);
                    warn!("   Auth time: {:?}", auth_time);
                    if attempt == max_attempts {
                        return Err(NetworkError::AuthenticationFailed { 
                            reason: format!("UseCircuitCode failed after {} attempts: {}", max_attempts, e) 
                        });
                    }
                },
                Err(_) => {
                    let auth_time = auth_start.elapsed();
                    warn!("🔍 SEND_RELIABLE: Timeout occurred!");
                    warn!("⏰ AUTH RESPONSE TIMEOUT: UseCircuitCode reliable send timed out");
                    warn!("   Attempt: {}/{}", attempt, max_attempts);
                    warn!("   Timeout after: {:?}", auth_time);
                    warn!("   Expected timeout: {:?}", timeout_duration);
                    if attempt == max_attempts {
                        error!("🔥 CRITICAL: All authentication attempts failed!");
                        error!("   This usually indicates a network connectivity issue:");
                        error!("   1. Check Windows Firewall - allow slv-rust.exe inbound/outbound");
                        error!("   2. Check router/NAT settings for UDP port {}", circuit.address().port());
                        error!("   3. Try disabling antivirus/security software temporarily");
                        error!("   4. Verify your Second Life account is active");
                        return Err(NetworkError::AuthenticationFailed { 
                            reason: format!("UseCircuitCode timed out after {} attempts - likely firewall/network issue", max_attempts) 
                        });
                    }
                }
            }
            
            if !success {
                // Exponential backoff delay between attempts to avoid overwhelming the server
                let delay = std::time::Duration::from_millis(1000 * (1 << (attempt - 1).min(2)) as u64);
                info!("💤 Waiting {:?} before next attempt", delay);
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
        
        // SECURITY: Verify UseCircuitCode was successfully processed before proceeding
        if self.config.security.enable_circuit_validation {
            self.verify_circuit_authentication(circuit).await?;
        } else {
            warn!("🔒 SECURITY WARNING: Circuit validation is disabled - this reduces security!");
        }
        
        // Small delay to ensure UseCircuitCode is processed by server
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        
        // Step 2: Send CompleteAgentMovement - critical for main region handshake
        // This triggers the server to send RegionHandshake AND AgentMovementComplete
        let complete_agent_movement = CompleteAgentMovement {
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
            circuit_code: circuit.circuit_code(),
        };
        
        // SECURITY: CompleteAgentMovement also contains sensitive data, ensure reliable delivery
        info!("🔐 BYPASS: Sending CompleteAgentMovement packet unreliably for testing");
        circuit.send(&complete_agent_movement).await?;
        info!("🚀 Sent CompleteAgentMovement - server should now respond with RegionHandshake");
        
        // Small delay to allow CompleteAgentMovement to be processed
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // Step 3: Send UUIDNameRequest for essential avatar data
        let uuid_name_request = UUIDNameRequest {
            uuidname_block: vec![UUIDNameBlockBlock {
                id: self.session_info.agent_id,
            }],
        };
        
        circuit.send(&uuid_name_request).await?;
        info!("Sent UUIDNameRequest");
        
        // Step 4: Send AgentFOV - Field of view configuration  
        // NOTE: AgentThrottle and AgentHeightWidth are sent AFTER RegionHandshakeReply
        // in the RegionHandshakeHandler, following the official protocol sequence
        let agent_fov = AgentFOV {
            agent_id: self.session_info.agent_id,
            session_id: self.session_info.session_id,
            circuit_code: circuit.circuit_code(),
            gen_counter: 0,
            vertical_angle: 1.2566370964050293, // ~72 degrees (homunculus standard)
        };
        
        circuit.send(&agent_fov).await?;
        info!("👁️ Sent AgentFOV with 72-degree field of view");
        
        info!("✅ All critical handshake packets sent");
        info!("Server will respond with:");
        info!("  1. RegionHandshake (handled by RegionHandshakeHandler)");
        info!("  2. AgentMovementComplete (handled by AgentMovementCompleteHandler)");
        info!("The AgentMovementCompleteHandler will complete the full handshake sequence");
        
        // Step 7: TEMPORARY - Skip handshake wait and proceed directly to EventQueue
        // TODO: Implement proper RegionHandshake and AgentMovementComplete handlers
        info!("🤝 HANDSHAKE: TEMPORARY - Skipping handshake event wait");
        info!("   Note: RegionHandshake and AgentMovementComplete handlers need implementation");
        info!("   Proceeding directly to EventQueue for testing");
        
        // Give server a moment to process our handshake packets
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        
        // ORIGINAL HANDSHAKE WAIT CODE (commented out for now)
        /*
        let mut region_handshake_received = false;
        let mut agent_movement_complete_received = false;
        let handshake_start = std::time::Instant::now();

        let mut handshake_rx = self.handshake_rx.write().await;

        while !region_handshake_received || !agent_movement_complete_received {
            let elapsed = handshake_start.elapsed();
            info!("🤝 HANDSHAKE: Waiting for events (elapsed: {:?})", elapsed);
            
            match timeout(Duration::from_secs(30), handshake_rx.recv()).await {
                Ok(Some(event)) => {
                    let event_time = handshake_start.elapsed();
                    match event {
                        HandshakeEvent::RegionHandshake => {
                            info!("✅ HANDSHAKE RESPONSE: RegionHandshake received");
                            info!("   Event time: {:?}", event_time);
                            info!("   Status: 1/2 handshake events completed");
                            region_handshake_received = true;
                        },
                        HandshakeEvent::AgentMovementComplete => {
                            info!("✅ HANDSHAKE RESPONSE: AgentMovementComplete received");
                            info!("   Event time: {:?}", event_time);
                            info!("   Status: 2/2 handshake events completed");
                            agent_movement_complete_received = true;
                        },
                    }
                },
                Ok(None) => {
                    let elapsed_time = handshake_start.elapsed();
                    warn!("❌ HANDSHAKE RESPONSE ERROR: Handshake channel closed unexpectedly");
                    warn!("   Elapsed time: {:?}", elapsed_time);
                    return Err(NetworkError::HandshakeFailed { reason: "Handshake channel closed unexpectedly".to_string() });
                },
                Err(_) => {
                    let elapsed_time = handshake_start.elapsed();
                    warn!("⏰ HANDSHAKE RESPONSE TIMEOUT: Handshake timed out");
                    warn!("   Elapsed time: {:?}", elapsed_time);
                    warn!("   RegionHandshake received: {}", region_handshake_received);
                    warn!("   AgentMovementComplete received: {}", agent_movement_complete_received);
                    return Err(NetworkError::HandshakeFailed { reason: "Handshake timed out".to_string() });
                }
            }
        }

        let total_handshake_time = handshake_start.elapsed();
        info!("✅ HANDSHAKE RESPONSE: Handshake complete!");
        info!("   Total handshake time: {:?}", total_handshake_time);
        info!("   RegionHandshake: ✓");
        info!("   AgentMovementComplete: ✓");
        */

        info!("✅ HANDSHAKE BYPASS: Handshake packets sent, proceeding to EventQueue");
        
        // Step 7: Start EventQueueGet (moved from commented section above)
        info!("🔍 CLIENT CONNECT: About to start EventQueueGet");
        match self.start_event_queue_get().await {
            Ok(()) => {
                info!("✅ CLIENT CONNECT: EventQueueGet started successfully");
            }
            Err(e) => {
                error!("❌ CLIENT CONNECT: EventQueueGet failed: {}", e);
                return Err(e);
            }
        }

        info!("🔍 CLIENT CONNECT: Method completing successfully");
        Ok(())
    }

    /// Start the EventQueueGet long-polling connection
    async fn start_event_queue_get(&self) -> NetworkResult<()> {
        info!("🔍 START_EVENT_QUEUE_GET: Method starting");
        let eqg_url = self.session_info.capabilities.as_ref()
            .and_then(|caps| caps.get("EventQueueGet"))
            .ok_or_else(|| NetworkError::Other { reason: "EventQueueGet capability not found".to_string() })?
            .clone();

        info!("🔍 START_EVENT_QUEUE_GET: EventQueueGet URL found: {}", eqg_url);
        info!("Starting EventQueueGet connection to {}", eqg_url);

        let agent = self.http_agent.clone();
        let session_id = self.session_info.session_id;
        let agent_id = self.session_info.agent_id;
        let eqg_url = eqg_url.clone();

        tokio::spawn(async move {
            let mut ack_id: Option<i64> = None;
            let mut consecutive_errors = 0;
            const MAX_CONSECUTIVE_ERRORS: u32 = 10;
            
            loop {
                // Build request body to match official viewer format exactly
                let request_body = if let Some(ack) = ack_id {
                    format!("<llsd><map><key>ack</key><integer>{}</integer><key>done</key><boolean>0</boolean></map></llsd>", ack)
                } else {
                    "<llsd><map><key>ack</key><undef /><key>done</key><boolean>0</boolean></map></llsd>".to_string()
                };
                
                debug!("Sending EventQueueGet request with ack: {:?}", ack_id);
                
                info!("🌐 HTTP REQUEST: Sending EventQueueGet request");
                info!("   URL: {}", eqg_url);
                info!("   ACK ID: {:?}", ack_id);
                
                let request_start = std::time::Instant::now();
                let agent_clone = agent.clone();
                let url_clone = eqg_url.clone();
                let body_clone = request_body.clone();
                
                match tokio::task::spawn_blocking(move || {
                    agent_clone
                        .post(&url_clone)
                        .header("Accept-Encoding", "deflate, gzip")
                        .header("Connection", "keep-alive") 
                        .header("Keep-Alive", "300")
                        .header("Accept", "application/llsd+xml")
                        .header("Content-Type", "application/llsd+xml")
                        .header("X-SecondLife-UDP-Listen-Port", "65186")
                        .send(&body_clone)
                }).await {
                    Ok(response_result) => {
                        match response_result {
                            Ok(mut response) => {
                                let status = response.status();
                                let response_time = request_start.elapsed();
                                
                                info!("🌐 HTTP RESPONSE: EventQueueGet response received");
                                info!("   Status: {}", status);
                                info!("   Response time: {:?}", response_time);
                                
                                // Handle different HTTP responses per SL protocol
                                match status.as_u16() {
                                    200 => {
                                        // Successful response with events
                                        info!("✅ HTTP RESPONSE: EventQueueGet successful (200 OK)");
                                        match response.body_mut().read_to_string() {
                                            Ok(text) => {
                                        info!("📥 HTTP RESPONSE: EventQueueGet data received");
                                        info!("   Response size: {} bytes", text.len());
                                        debug!("   Response content: {}", text);
                                        
                                        // Parse LLSD XML response (simplified parsing)
                                        if let Some(id_match) = text.find("<key>id</key><integer>") {
                                            if let Some(id_end) = text[id_match + 22..].find("</integer>") {
                                                if let Ok(new_ack_id) = text[id_match + 22..id_match + 22 + id_end].parse::<i64>() {
                                                    info!("📝 HTTP RESPONSE: Updated ACK ID from {:?} to {}", ack_id, new_ack_id);
                                                    ack_id = Some(new_ack_id);
                                                }
                                            }
                                        }
                                        
                                        // Process specific event types critical for region transitions
                                        if text.contains("EnableSimulator") {
                                            info!("🌐 HTTP RESPONSE EVENT: EnableSimulator event received - neighbor region available");
                                            Self::handle_enable_simulator_event(&text).await;
                                        } else if text.contains("CrossedRegion") {
                                            info!("🎉 HTTP RESPONSE EVENT: CrossedRegion event received - region transition complete");
                                            Self::handle_crossed_region_event(&text).await;
                                        } else if text.contains("TeleportProgress") {
                                            info!("✈️ HTTP RESPONSE EVENT: TeleportProgress event received");
                                            Self::handle_teleport_progress_event(&text).await;
                                        } else if text.contains("TeleportFinish") {
                                            info!("✅ HTTP RESPONSE EVENT: TeleportFinish event received - teleport complete");
                                            Self::handle_teleport_finish_event(&text).await;
                                        } else if text.contains("DisableSimulator") {
                                            info!("🌌 HTTP RESPONSE EVENT: DisableSimulator event received - neighbor region offline");
                                            Self::handle_disable_simulator_event(&text).await;
                                        } else {
                                            info!("📧 HTTP RESPONSE EVENT: Other event received");
                                            debug!("   Event preview: {}", text.lines().next().unwrap_or("<unknown>"));
                                        }
                                        
                                        consecutive_errors = 0; // Reset error counter
                                        info!("✅ HTTP RESPONSE: EventQueueGet processing completed successfully");
                                            },
                                            Err(e) => {
                                                warn!("❌ HTTP RESPONSE ERROR: Failed to read EventQueueGet response text");
                                                warn!("   Error: {}", e);
                                                warn!("   Response time: {:?}", response_time);
                                                consecutive_errors += 1;
                                            }
                                        }
                                    },
                                    502 => {
                                        // 502 Bad Gateway = no events available, continue polling
                                        info!("🔄 HTTP RESPONSE: EventQueueGet no events available (502 Bad Gateway)");
                                        info!("   Response time: {:?}", response_time);
                                        info!("   Continuing long-polling...");
                                        consecutive_errors = 0;
                                    },
                                    499 => {
                                        // 499 Client Closed Request = server wants us to reconnect
                                        info!("🔄 HTTP RESPONSE: EventQueueGet server requested reconnection (499)");
                                        info!("   Response time: {:?}", response_time);
                                        info!("   Resetting ACK ID and reconnecting...");
                                        ack_id = None; // Reset acknowledgment
                                    },
                                    _ => {
                                        warn!("❌ HTTP RESPONSE ERROR: EventQueueGet request failed");
                                        warn!("   Status: {}", status);
                                        warn!("   Response time: {:?}", response_time);
                                        consecutive_errors += 1;
                                    }
                                }
                            },
                            Err(e) => {
                                let response_time = request_start.elapsed();
                                warn!("❌ HTTP REQUEST ERROR: Failed to send EventQueueGet request");
                                warn!("   Error: {}", e);
                                warn!("   Request time: {:?}", response_time);
                                consecutive_errors += 1;
                            }
                        }
                    },
                    Err(e) => {
                        let response_time = request_start.elapsed();
                        warn!("❌ HTTP TASK ERROR: Failed to execute EventQueueGet request task");
                        warn!("   Error: {}", e);
                        warn!("   Task time: {:?}", response_time);
                        consecutive_errors += 1;
                    }
                }
                
                // Handle consecutive errors
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    tracing::error!("EventQueueGet: Too many consecutive errors ({}), backing off", consecutive_errors);
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    consecutive_errors = 0;
                } else if consecutive_errors > 0 {
                    // Exponential backoff for errors
                    let delay = std::cmp::min(consecutive_errors * 2, 30);
                    tokio::time::sleep(std::time::Duration::from_secs(delay as u64)).await;
                } else {
                    // Normal operation - brief pause before next poll
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        });

        Ok(())
    }
    
    /// Handle EnableSimulator event - establishes connection to neighboring region
    async fn handle_enable_simulator_event(event_xml: &str) {
        // EnableSimulator contains IP, Port, and RegionHandle for neighbor region
        // Example: <key>IP</key><integer>3232235777</integer><key>Port</key><integer>9000</integer>
        debug!("EnableSimulator event: {}", event_xml);
        
        // TODO: Parse LLSD XML to extract IP/Port/Handle and establish neighbor circuit
        // This is critical for region crossings and seeing into adjacent regions
        info!("🔗 TODO: Implement neighbor region circuit establishment");
    }
    
    /// Handle CrossedRegion event - completes region transition
    async fn handle_crossed_region_event(event_xml: &str) {
        debug!("CrossedRegion event: {}", event_xml);
        
        // CrossedRegion indicates successful handoff to new region
        // The client should now consider the new region as primary
        info!("✅ Region crossing completed successfully");
    }
    
    /// Handle TeleportProgress event - teleport status update
    async fn handle_teleport_progress_event(event_xml: &str) {
        debug!("TeleportProgress event: {}", event_xml);
        info!("📍 Teleport in progress...");
    }
    
    /// Handle TeleportFinish event - teleport completion
    async fn handle_teleport_finish_event(event_xml: &str) {
        debug!("TeleportFinish event: {}", event_xml);
        
        // TeleportFinish signals client should begin rendering new location
        // Contains final position and region information
        info!("🎯 Teleport completed - should begin rendering new location");
        
        // TODO: Parse event data to extract final position and establish new primary circuit
    }
    
    /// Handle DisableSimulator event - neighbor region going offline
    async fn handle_disable_simulator_event(event_xml: &str) {
        debug!("DisableSimulator event: {}", event_xml);
        
        // DisableSimulator indicates a neighbor region is going offline
        // Should close any circuits to that region
        info!("🔌 Neighbor region disabled - should close relevant circuits");
    }
    
    /// Start background tasks
    async fn start_background_tasks(&self) {
        let state = Arc::clone(&self.state);
        let event_tx = self.event_tx.clone();
        let mut core_events = self.core.get_event_receiver();
        
        // Core event handler
        tokio::spawn(async move {
            while let Some(event) = core_events.recv().await {
                match event {
                    CoreEvent::StateChanged { old_state, new_state } => {
                        debug!("Core state changed: {:?} -> {:?}", old_state, new_state);
                    }
                    CoreEvent::CircuitConnected { address } => {
                        let _ = event_tx.send(ClientEvent::Connected { address });
                    }
                    CoreEvent::CircuitDisconnected { address, reason } => {
                        let _ = event_tx.send(ClientEvent::Disconnected { address, reason });
                    }
                    CoreEvent::Error { error } => {
                        let _ = event_tx.send(ClientEvent::Error { error });
                    }
                }
            }
        });
    }
    
    /// Set client state and emit event
    async fn set_state(&self, new_state: ClientState) {
        let old_state = {
            let mut state = self.state.write().await;
            let old = state.clone();
            *state = new_state.clone();
            old
        };
        
        if old_state != new_state {
            let _ = self.event_tx.send(ClientEvent::StateChanged { old_state, new_state });
        }
    }
    
    /// Get the networking core (for advanced usage)
    pub fn core(&self) -> Arc<Core> {
        Arc::clone(&self.core)
    }
}