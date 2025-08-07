use super::{Grid, SessionInfo, SessionManager, CredentialStore};
use super::xmlrpc::{XmlRpcClient, LoginParameters};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::client::{Client, ClientConfig};
use crate::ui::proxy::ProxySettings;
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;
use reqwest::{Client as ReqwestClient, Certificate};
use std::fs;
use anyhow::{Context, Result};
use std::collections::HashMap;
use serde_json;
use quick_xml;
use crate::utils::math::Vector3;

#[derive(Debug, Clone)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
    pub grid: Grid,
    pub start_location: String,
}

impl LoginCredentials {
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password,
            grid: Grid::default(),
            start_location: "last".to_string(),
        }
    }
    
    pub fn with_grid(mut self, grid: Grid) -> Self {
        self.grid = grid;
        self
    }
    
    pub fn with_start_location(mut self, location: String) -> Self {
        self.start_location = location;
        self
    }
    
    pub fn validate(&self) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        
        if self.password.trim().is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        
        // Username format validation - allow firstname, firstname.lastname, or firstname lastname
        let trimmed_username = self.username.trim();
        if trimmed_username.is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        
        // Check for valid characters only
        if !trimmed_username.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '.') {
            return Err("Username can only contain letters, numbers, spaces, and periods".to_string());
        }
        
        Ok(())
    }

    /// Split username into first and last name
    /// Handles formats: "firstname", "firstname.lastname", "firstname lastname"
    pub fn split_name(&self) -> (String, String) {
        let username = self.username.trim();
        
        // Handle firstname.lastname format - convert to firstname lastname
        if username.contains('.') && !username.contains(' ') {
            let parts: Vec<&str> = username.splitn(2, '.').collect();
            match parts.as_slice() {
                [first, last] => (first.to_string(), last.to_string()),
                [first] => (first.to_string(), "Resident".to_string()),
                _ => ("Unknown".to_string(), "User".to_string()),
            }
        } else {
            // Handle firstname lastname or firstname alone
            let parts: Vec<&str> = username.splitn(2, ' ').collect();
            match parts.as_slice() {
                [first] => (first.to_string(), "Resident".to_string()),
                [first, last] => (first.to_string(), last.to_string()),
                _ => ("Unknown".to_string(), "User".to_string()),
            }
        }
    }
}

pub struct AuthenticationService {
    session_manager: SessionManager,
    xmlrpc_client: XmlRpcClient,
    credential_store: CredentialStore,
    proxy_settings: Option<ProxySettings>,
    // Cached HTTP client to avoid recreating it for every request
    http_client: ReqwestClient,
}

impl AuthenticationService {
    pub fn new() -> Self {
        let http_client = Self::build_proxied_client(None);
        Self {
            session_manager: SessionManager::new(),
            xmlrpc_client: XmlRpcClient::new(),
            credential_store: CredentialStore::new(),
            proxy_settings: None,
            http_client,
        }
    }

    /// Create authentication service with proxy configuration
    pub fn new_with_proxy(proxy_settings: &ProxySettings) -> NetworkResult<Self> {
        if proxy_settings.enabled {
            tracing::info!("🔧 Creating AuthenticationService with proxy enabled");
            tracing::info!("  - HTTP proxy: {}:{}", proxy_settings.http_host, proxy_settings.http_port);
            tracing::info!("  - Cert validation disabled: {}", proxy_settings.disable_cert_validation);
            
            let xmlrpc_client = XmlRpcClient::new_with_proxy(
                Some(proxy_settings)
            ).map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to configure proxy for authentication: {}", e) 
            })?;

            let http_client = Self::build_proxied_client(Some(proxy_settings));
            Ok(Self {
                session_manager: SessionManager::new(),
                xmlrpc_client,
                credential_store: CredentialStore::new(),
                proxy_settings: Some(proxy_settings.clone()),
                http_client,
            })
        } else {
            tracing::info!("🔧 Creating AuthenticationService without proxy");
            Ok(Self::new())
        }
    }

    /// Build a reqwest client with proxy settings if enabled (like main branch)
    fn build_proxied_client(proxy_settings: Option<&ProxySettings>) -> ReqwestClient {
        let mut builder = ReqwestClient::builder();
        
        if let Some(proxy_cfg) = proxy_settings {
            if proxy_cfg.enabled {
                // Always use the proxy when enabled
                let proxy_url = format!("http://{}:{}", proxy_cfg.http_host, proxy_cfg.http_port);
                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    builder = builder.proxy(proxy);
                }
                
                // Add CA certificate for Hippolyzer
                if let Ok(ca_cert) = fs::read("src/assets/CA.pem") {
                    if let Ok(cert) = Certificate::from_pem(&ca_cert) {
                        builder = builder.add_root_certificate(cert);
                    }
                }
            }
        }
        
        builder
            .timeout(std::time::Duration::from_secs(60))
            .user_agent("Second Life Release 7.1.15 (1559633637437)")
            .build()
            .expect("Failed to build HTTP client")
    }

    /// Perform OpenID POST authentication (similar to main branch implementation)
    /// This now sends TWO OpenID requests to match the official viewer sequence:
    /// 1. First request (same as before)
    /// 2. Second request with X-SecondLife-UDP-Listen-Port header
    async fn perform_openid_post(&self, openid_token: &str, openid_url: &str) -> NetworkResult<()> {
        tracing::info!("🔐 Sending first OpenID token to {}", openid_url);
        tracing::debug!("🔐 OpenID token: {}", openid_token);

        let client = &self.http_client;

        // First OpenID request (same as before)
        
        let response = client
            .post(openid_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "*/*")
            .header("Accept-Encoding", "gzip, deflate")
            .header("Connection", "keep-alive")
            .header("Keep-Alive", "300")
            .body(openid_token.to_string())
            .send()
            .await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to send first OpenID request: {}", e) })?;

        let status_code = response.status().as_u16();
        let response_text = response.text().await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to read first OpenID response: {}", e) })?;

        if status_code < 200 || status_code >= 300 {
            tracing::warn!("⚠️ First OpenID POST returned status {}: {}", status_code, 
                          response_text.chars().take(200).collect::<String>());
            return Err(NetworkError::Transport { 
                reason: format!("First OpenID POST failed with status: {}", status_code) 
            });
        }

        tracing::info!("✅ First OpenID POST completed successfully (status {})", status_code);

        // Second OpenID request with UDP listen port header (matching official viewer)
        tracing::info!("🔐 Sending second OpenID token with UDP listen port");
        
        // Create a temporary UDP socket to get the actual port we'll be listening on
        // This matches what the networking client will use later
        let temp_socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to bind UDP socket for port detection: {}", e) 
            })?;
        let udp_port = temp_socket.local_addr()
            .map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to get local UDP port: {}", e) 
            })?
            .port();
        
        tracing::info!("🔐 Using actual UDP listen port: {}", udp_port);
        
        let url_clone2 = openid_url.to_string();
        let token_clone2 = openid_token.to_string();
        
        let response2 = client.clone()
            .post(openid_url)
            .header("Host", "id.secondlife.com")
            .header("Accept-Encoding", "deflate, gzip")
            .header("Connection", "keep-alive")
            .header("Keep-alive", "300")
            .header("Accept", "*/*")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("X-SecondLife-UDP-Listen-Port", &udp_port.to_string())
            .header("Content-Length", &openid_token.len().to_string())
            .body(openid_token.to_string())
            .send()
            .await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to send second OpenID request: {}", e) })?;

        let status_code2 = response2.status().as_u16();
        let response_text2 = response2.text().await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to read second OpenID response: {}", e) })?;

        if status_code2 < 200 || status_code2 >= 300 {
            tracing::warn!("⚠️ Second OpenID POST returned status {}: {}", status_code2, 
                          response_text2.chars().take(200).collect::<String>());
            return Err(NetworkError::Transport { 
                reason: format!("Second OpenID POST failed with status: {}", status_code2) 
            });
        }

        tracing::info!("✅ Second OpenID POST completed successfully (status {})", status_code2);
        tracing::info!("✅ OpenID authentication sequence completed (matching official viewer)");
        Ok(())
    }

    /// Fetch additional capabilities that the official viewer requests during login
    /// This includes navigation mesh status, environment data, and other critical capabilities
    async fn fetch_additional_capabilities(&self, capabilities: &HashMap<String, String>) -> NetworkResult<()> {
        tracing::info!("🔍 FETCHING: Additional capabilities to match official viewer behavior");
        
        // 1. Navigation Mesh Status - Critical for avatar movement
        if let Some(nav_mesh_url) = capabilities.get("NavMeshGenerationStatus") {
            tracing::info!("🗺️ Fetching navigation mesh status from: {}", nav_mesh_url);
            match self.fetch_capability_data(nav_mesh_url, "navigation mesh status").await {
                Ok(_) => tracing::info!("✅ Navigation mesh status fetched successfully"),
                Err(e) => tracing::warn!("⚠️ Navigation mesh status fetch failed: {}", e),
            }
        }
        
        // 2. Environment Settings - World lighting and environment
        if let Some(env_url) = capabilities.get("EnvironmentSettings") {
            tracing::info!("🌍 Fetching environment settings from: {}", env_url);
            match self.fetch_capability_data(env_url, "environment settings").await {
                Ok(_) => tracing::info!("✅ Environment settings fetched successfully"),
                Err(e) => tracing::warn!("⚠️ Environment settings fetch failed: {}", e),
            }
        }
        
        // 3. Agent Preferences - User-specific settings
        if let Some(prefs_url) = capabilities.get("AgentPreferences") {
            tracing::info!("👤 Fetching agent preferences from: {}", prefs_url);
            match self.fetch_capability_data(prefs_url, "agent preferences").await {
                Ok(_) => tracing::info!("✅ Agent preferences fetched successfully"),  
                Err(e) => tracing::warn!("⚠️ Agent preferences fetch failed: {}", e),
            }
        }
        
        // 4. Map Image Download - For minimap functionality  
        if let Some(map_url) = capabilities.get("MapLayer") {
            tracing::info!("🗺️ Map layer capability available: {}", map_url);
            // Don't actually download map tiles now, but log availability
        }
        
        tracing::info!("✅ Additional capability fetching completed");
        Ok(())
    }
    
    /// Helper method to fetch data from a capability URL using reqwest
    async fn fetch_capability_data(&self, url: &str, description: &str) -> NetworkResult<String> {
        use crate::networking::proxy::http::HttpProxyClient;
        
        tracing::debug!("🔗 Using cached HTTP client for {} request to {}", description, url);
        let client = &self.http_client;

        let response = client
            .get(url)
            .header("Accept", "application/llsd+xml")
            .header("User-Agent", "Second Life Release 7.1.15 (1559633637437)")
            .send()
            .await
            .map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to fetch {}: {}", description, e) 
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(NetworkError::Transport { 
                reason: format!("Failed to fetch {}: HTTP {}", description, status) 
            });
        }

        let response_text = response.text().await
            .map_err(|e| NetworkError::Transport { 
                reason: format!("Failed to read {} response body: {}", description, e) 
            })?;

        tracing::debug!("📊 {} response: {}", description, response_text.chars().take(200).collect::<String>());
        Ok(response_text)
    }

    async fn fetch_capabilities(&self, url: &str) -> NetworkResult<HashMap<String, String>> {
        use crate::networking::capabilities::seed::SeedCapabilityClient;
        
        tracing::info!("🌱 Fetching comprehensive capabilities from {}", url);
        
        // Use the comprehensive seed capability client that matches official viewer behavior
        let seed_client = SeedCapabilityClient::new_with_proxy(self.proxy_settings.as_ref());
        seed_client.fetch_capabilities(url).await
    }

    fn parse_llsd_xml(&self, xml_text: &str) -> NetworkResult<HashMap<String, String>> {
        use quick_xml::Reader;
        use quick_xml::events::Event;
        use std::io::BufRead;

        let mut reader = Reader::from_str(xml_text);
        reader.trim_text(true);

        let mut capabilities = HashMap::new();
        let mut buf = Vec::new();
        let mut current_key: Option<String> = None;
        let mut current_value: Option<String> = None;
        let mut in_map = false;
        let mut depth = 0;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    match e.name().as_ref() {
                        b"llsd" => {
                            // Root element, continue
                        }
                        b"map" => {
                            in_map = true;
                            depth += 1;
                        }
                        b"key" => {
                            current_key = None;
                        }
                        b"string" => {
                            current_value = None;
                        }
                        _ => {
                            // Other elements, continue
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.name().as_ref() {
                        b"llsd" => {
                            break;
                        }
                        b"map" => {
                            depth -= 1;
                            if depth == 0 {
                                in_map = false;
                            }
                        }
                        b"key" => {
                            // Key parsing completed
                        }
                        b"string" => {
                            // String parsing completed, store the key-value pair
                            if let (Some(key), Some(value)) = (current_key.take(), current_value.take()) {
                                if in_map {
                                    capabilities.insert(key, value);
                                }
                            }
                        }
                        _ => {
                            // Other elements, continue
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if current_key.is_none() {
                        current_key = Some(text);
                    } else if current_value.is_none() {
                        current_value = Some(text);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(NetworkError::Transport { 
                        reason: format!("Failed to parse LLSD XML: {}", e) 
                    });
                }
                _ => {
                    // Other events, continue
                }
            }
            buf.clear();
        }

        Ok(capabilities)
    }
    
    pub async fn login(&mut self, credentials: LoginCredentials) -> NetworkResult<Client> {
        self.login_with_retry(credentials, 3, false).await
    }

    /// Login with proxy support
    pub async fn login_with_proxy(&mut self, credentials: LoginCredentials, proxy_enabled: bool) -> NetworkResult<Client> {
        self.login_with_retry(credentials, 3, proxy_enabled).await
    }

    /// Login with retry logic and fallback locations
    async fn login_with_retry(&mut self, mut credentials: LoginCredentials, max_retries: u32, proxy_enabled: bool) -> NetworkResult<Client> {
        // Validate credentials
        credentials.validate()
            .map_err(|e| NetworkError::AuthenticationFailed { reason: e })?;
        
        let fallback_locations = ["last", "home"];
        let original_location = credentials.start_location.clone();
        
        for attempt in 0..max_retries {
            // Use fallback location if available
            if attempt > 0 && (attempt - 1) < fallback_locations.len() as u32 {
                credentials.start_location = fallback_locations[(attempt - 1) as usize].to_string();
                tracing::info!("Attempting login with fallback start location: {}", credentials.start_location);
            }
            
            // Try current start location
            match self.attempt_login(&credentials, proxy_enabled).await {
                Ok(client) => return Ok(client),
                Err(NetworkError::SimulatorConnectionFailed { reason }) if attempt < max_retries - 1 => {
                    tracing::warn!("Login attempt {} failed with simulator connection error: {}. Retrying in {}ms...", 
                                   attempt + 1, reason, Self::retry_delay_ms(attempt));
                    
                    // Wait with exponential backoff
                    tokio::time::sleep(Duration::from_millis(Self::retry_delay_ms(attempt))).await;
                }
                Err(e) => return Err(e), // Non-retryable errors
            }
        }
        
        Err(NetworkError::SimulatorConnectionFailed { 
            reason: format!("Failed to connect after {} attempts", max_retries) 
        })
    }

    /// Calculate retry delay with exponential backoff
    fn retry_delay_ms(attempt: u32) -> u64 {
        std::cmp::min(1000 * 2_u64.pow(attempt), 8000) // Cap at 8 seconds
    }

    /// Attempt a single login
    async fn attempt_login(&mut self, credentials: &LoginCredentials, proxy_enabled: bool) -> NetworkResult<Client> {
        // Step 1: Authenticate with login server
        let login_response = self.authenticate_with_login_server(credentials).await?;
        
        if !login_response.success {
            let reason = login_response.reason
                .or(login_response.message)
                .unwrap_or_else(|| "Login failed".to_string());
            
            // Classify the error based on reason
            if reason.contains("slave-connect-failure") || reason.contains("region") {
                tracing::error!("Simulator connection failed: {}", reason);
                return Err(NetworkError::SimulatorConnectionFailed { reason });
            } else {
                tracing::error!("Login rejected: {}", reason);
                return Err(NetworkError::LoginRejected { reason });
            }
        }
        
        // Step 2: Store credentials in keychain after successful XML-RPC login
        tracing::info!("Login successful, storing credentials in keychain...");
        
        // Run keychain test first
        super::keychain_test::test_keychain();
        
        if let Err(e) = self.credential_store.store_credentials(credentials) {
            tracing::warn!("Failed to store credentials in keychain: {}", e);
            // Don't fail the login if keychain storage fails
        } else {
            tracing::info!("Successfully stored credentials in keychain for grid {}", 
                          credentials.grid.name());
        }

        // Step 2.5: Perform OpenID POST if token is available
        if let (Some(ref token), Some(ref url)) = (&login_response.openid_token, &login_response.openid_url) {
            tracing::info!("🔐 Performing OpenID authentication POST");
            if let Err(e) = self.perform_openid_post(token, url).await {
                tracing::warn!("⚠️ OpenID POST failed: {}", e);
                // Don't fail login if OpenID POST fails - it may not be critical
            }
        } else {
            tracing::info!("ℹ️ No OpenID token received, skipping OpenID authentication");
        }

        // Step 3: Create session info - prioritize capabilities hostname
        let simulator_address = if let Some(ref seed_url) = login_response.seed_capability {
            if let Ok(url) = url::Url::parse(seed_url) {
                if let Some(host) = url.host_str() {
                    tracing::info!("🔍 Capabilities URL parsed successfully");
                    tracing::info!("🔍 Extracted hostname: '{}'", host);
                    tracing::info!("🔍 Hostname length: {} chars", host.len());
                    tracing::info!("🔍 Port from XML-RPC: {}", login_response.simulator_port);
                    
                    // Use capabilities hostname with XML-RPC provided port
                    let preferred_addr = format!("{}:{}", host, login_response.simulator_port);
                    tracing::info!("🎯 Constructed address: '{}'", preferred_addr);
                    
                    // Resolve hostname to socket address
                    use std::net::ToSocketAddrs;
                    preferred_addr.to_socket_addrs()
                        .map_err(|e| {
                            let error_msg = format!("Failed to resolve hostname '{}': {}", preferred_addr, e);
                            tracing::error!("{}", error_msg);
                            NetworkError::SimulatorConnectionFailed { reason: error_msg }
                        })?
                        .next()
                        .ok_or_else(|| {
                            let error_msg = format!("No addresses resolved for hostname '{}'", preferred_addr);
                            tracing::error!("{}", error_msg);
                            NetworkError::SimulatorConnectionFailed { reason: error_msg }
                        })?
                } else {
                    // Fallback to XML-RPC address
                    let xmlrpc_addr = login_response.simulator_address().map_err(|e| {
                        let error_msg = format!("Could not resolve any simulator address: {}", e);
                        tracing::error!("{}", error_msg);
                        NetworkError::SimulatorConnectionFailed { reason: error_msg }
                    })?;
                    tracing::warn!("⚠️ Using XML-RPC address as fallback: {} (from sim_ip={}, sim_port={})", 
                                  xmlrpc_addr, login_response.simulator_ip, login_response.simulator_port);
                    xmlrpc_addr
                }
            } else {
                // Fallback to XML-RPC address
                let xmlrpc_addr = login_response.simulator_address().map_err(|e| {
                    let error_msg = format!("Could not resolve any simulator address: {}", e);
                    tracing::error!("{}", error_msg);
                    NetworkError::SimulatorConnectionFailed { reason: error_msg }
                })?;
                tracing::warn!("⚠️ Using XML-RPC address as fallback: {} (from sim_ip={}, sim_port={})", 
                              xmlrpc_addr, login_response.simulator_ip, login_response.simulator_port);
                xmlrpc_addr
            }
        } else {
            // No capabilities URL - use XML-RPC address
            let xmlrpc_addr = login_response.simulator_address().map_err(|e| {
                let error_msg = format!("No simulator address available: {}", e);
                tracing::error!("{}", error_msg);
                NetworkError::SimulatorConnectionFailed { reason: error_msg }
            })?;
            tracing::warn!("⚠️ No capabilities URL, using XML-RPC address: {} (from sim_ip={}, sim_port={})", 
                          xmlrpc_addr, login_response.simulator_ip, login_response.simulator_port);
            xmlrpc_addr
        };

        let session = SessionInfo {
            agent_id: login_response.agent_id,
            session_id: login_response.session_id,
            secure_session_id: login_response.secure_session_id,
            first_name: login_response.first_name,
            last_name: login_response.last_name,
            circuit_code: login_response.circuit_code,
            simulator_address,
            look_at: login_response.look_at,
            start_location: credentials.start_location.clone(),
            seed_capability: login_response.seed_capability.clone(),
            capabilities: if let Some(ref seed_cap_url) = login_response.seed_capability {
                tracing::info!("🔍 CAPABILITIES: About to fetch capabilities from seed URL");
                tracing::info!("🔍 CAPABILITIES: Seed URL: {}", seed_cap_url);
                let caps_result = self.fetch_capabilities(&seed_cap_url).await;
                match &caps_result {
                    Ok(caps) => {
                        tracing::info!("✅ CAPABILITIES: Successfully fetched {} capabilities", caps.len());
                        tracing::info!("✅ CAPABILITIES: Available capabilities: {:?}", caps.keys().collect::<Vec<_>>());
                        
                        // Step 3.5: Fetch additional capabilities that the official viewer fetches
                        self.fetch_additional_capabilities(&caps).await?;
                    }
                    Err(e) => {
                        tracing::error!("❌ CAPABILITIES: Failed to fetch capabilities: {}", e);
                    }
                }
                Some(caps_result?)
            } else {
                tracing::warn!("⚠️ CAPABILITIES: No seed capability URL provided in login response");
                None
            },
        };
        
        // Step 4: Start session
        self.session_manager.start_session(session.clone());
        
        // Step 5: Create networking client
        let config = ClientConfig {
            agent_id: session.agent_id,
            session_id: session.session_id,
            ..Default::default()
        };
        
        let client = if proxy_enabled {
            tracing::info!("🔧 Creating client with Hippolyzer proxy support");
            Client::new_with_hippolyzer_proxy(config, session.clone()).await?
        } else {
            Client::new(config, session.clone()).await?
        };
        
        // Step 6: Connect to simulator with fallback
        tracing::info!("🔍 CLIENT CONNECT: About to call client.connect()");
        tracing::info!("🔍 CLIENT CONNECT: Simulator address: {}", session.simulator_address);
        tracing::info!("🔍 CLIENT CONNECT: Circuit code: {}", session.circuit_code);
        match client.connect(session.simulator_address, session.circuit_code).await {
            Ok(()) => {
                tracing::info!("Successfully connected to primary simulator address: {}", session.simulator_address);
            }
            Err(e) => {
                tracing::warn!("Primary simulator connection failed: {}. Attempting fallback...", e);
                
                // Fallback: try connecting to capabilities hostname with common SL ports
                if let Some(ref seed_url) = login_response.seed_capability {
                    if let Ok(url) = url::Url::parse(seed_url) {
                        if let Some(host) = url.host_str() {
                            // Try common Second Life UDP ports
                            let fallback_ports = [9000, 9001, 9002, 12043, 13000, 13001];
                            let mut connected = false;
                            
                            for &port in &fallback_ports {
                                let fallback_addr = format!("{}:{}", host, port);
                                tracing::info!("Trying fallback simulator address: {}", fallback_addr);
                                
                                if let Ok(addr) = fallback_addr.parse() {
                                    match client.connect(addr, session.circuit_code).await {
                                        Ok(()) => {
                                            tracing::info!("✅ Successfully connected to fallback simulator: {}", fallback_addr);
                                            connected = true;
                                            break;
                                        }
                                        Err(fallback_error) => {
                                            tracing::debug!("Fallback {} failed: {}", fallback_addr, fallback_error);
                                        }
                                    }
                                }
                            }
                            
                            if !connected {
                                return Err(NetworkError::SimulatorConnectionFailed { 
                                    reason: format!("Both primary ({}) and all fallback addresses failed", session.simulator_address)
                                });
                            }
                        } else {
                            return Err(e);
                        }
                    } else {
                        return Err(e);
                    }
                } else {
                    return Err(e);
                }
            }
        }
        
        tracing::info!("✅ ATTEMPT LOGIN: Returning successful client");
        Ok(client)
    }
    
    pub fn logout(&mut self) {
        self.session_manager.end_session();
    }
    
    pub fn current_session(&self) -> Option<&SessionInfo> {
        self.session_manager.current_session()
    }
    
    pub fn is_logged_in(&self) -> bool {
        self.session_manager.is_logged_in()
    }

    pub fn has_stored_credentials(&self, grid_name: &str) -> bool {
        self.credential_store.has_stored_credentials(grid_name)
    }

    pub fn load_stored_credentials(&self, grid_name: &str) -> Option<LoginCredentials> {
        match self.credential_store.load_credentials(grid_name) {
            Ok(credentials) => credentials,
            Err(e) => {
                tracing::warn!("Failed to load stored credentials for grid {}: {}", grid_name, e);
                None
            }
        }
    }

    pub fn delete_stored_credentials(&self, grid_name: &str) -> bool {
        match self.credential_store.delete_credentials(grid_name) {
            Ok(()) => {
                tracing::info!("Successfully deleted stored credentials for grid {}", grid_name);
                true
            }
            Err(e) => {
                tracing::warn!("Failed to delete stored credentials for grid {}: {}", grid_name, e);
                false
            }
        }
    }
    
    async fn authenticate_with_login_server(&self, credentials: &LoginCredentials) -> NetworkResult<super::LoginResponse> {
        let (first_name, last_name) = credentials.split_name();
        
        let mut params = LoginParameters::new(&first_name, &last_name, &credentials.password);
        params.start_location = credentials.start_location.clone();
        
        let login_uri = credentials.grid.login_uri();
        
        tracing::info!("🚀 Beginning authentication process");
        tracing::info!("  - Grid: {}", credentials.grid.name());
        tracing::info!("  - Login URI: {}", login_uri);
        tracing::info!("  - Username: {}.{}", first_name, last_name);
        tracing::info!("  - Start location: {}", credentials.start_location);
        
        let auth_start_time = std::time::Instant::now();
        
        let result = self.xmlrpc_client.login_to_simulator(login_uri, params).await;
        
        let auth_elapsed = auth_start_time.elapsed();
        
        match &result {
            Ok(response) => {
                tracing::info!("🎉 Authentication completed successfully in {:?}", auth_elapsed);
                tracing::info!("  - Agent ID: {}", response.agent_id);
                tracing::info!("  - Session ID: {}", response.session_id);
                tracing::info!("  - Circuit code: {}", response.circuit_code);
                tracing::info!("  - Simulator: {}:{}", response.simulator_ip, response.simulator_port);
                if let Some(ref seed_cap) = response.seed_capability {
                    tracing::info!("  - Seed capability: {}", seed_cap);
                }
            }
            Err(e) => {
                tracing::error!("💥 Authentication failed after {:?}: {}", auth_elapsed, e);
            }
        }
        
        result.map_err(|e| NetworkError::AuthenticationFailed { 
            reason: format!("Login server communication failed: {}", e) 
        })
    }

}

impl Default for AuthenticationService {
    fn default() -> Self {
        Self::new()
    }
}