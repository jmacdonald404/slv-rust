use super::{Grid, SessionInfo, SessionManager, CredentialStore};
use super::xmlrpc::{XmlRpcClient, LoginParameters};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::client::{Client, ClientConfig};
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;
use reqwest::Client as HttpClient;
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
}

impl AuthenticationService {
    pub fn new() -> Self {
        Self {
            session_manager: SessionManager::new(),
            xmlrpc_client: XmlRpcClient::new(),
            credential_store: CredentialStore::new(),
        }
    }

    async fn fetch_capabilities(&self, url: &str) -> NetworkResult<HashMap<String, String>> {
        tracing::info!("Fetching capabilities from {}", url);
        let client = HttpClient::new();
        
        // Second Life capabilities servers expect POST requests with LLSD format
        // Send an empty LLSD map as the request body
        let request_body = r#"<?xml version="1.0" ?><llsd><map></map></llsd>"#;
        
        let response = client.post(url)
            .header("Content-Type", "application/llsd+xml")
            .body(request_body)
            .send().await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to fetch capabilities: {}", e) })?;

        if !response.status().is_success() {
            return Err(NetworkError::Transport { reason: format!("Failed to fetch capabilities, status: {}", response.status()) });
        }

        // Get the response text first for debugging
        let response_text = response.text().await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to read capabilities response: {}", e) })?;

        tracing::debug!("Capabilities response: {}", response_text);

        // Try to parse as JSON first, then as LLSD if JSON fails
        let capabilities: HashMap<String, String> = if let Ok(json) = serde_json::from_str::<HashMap<String, String>>(&response_text) {
            json
        } else {
            // If JSON parsing fails, try to parse as LLSD (Linden Lab Structured Data)
            self.parse_llsd_xml(&response_text)?
        };

        Ok(capabilities)
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
                Some(self.fetch_capabilities(&seed_cap_url).await?)
            } else {
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
        
        tracing::info!("Authenticating with {} at {}", credentials.grid.name(), login_uri);
        
        self.xmlrpc_client.login_to_simulator(login_uri, params)
            .await
            .map_err(|e| NetworkError::AuthenticationFailed { 
                reason: format!("Login server communication failed: {}", e) 
            })
    }
}

impl Default for AuthenticationService {
    fn default() -> Self {
        Self::new()
    }
}