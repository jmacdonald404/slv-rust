use super::{Grid, SessionInfo, SessionManager};
use super::xmlrpc::{XmlRpcClient, LoginParameters};
use crate::networking::{NetworkError, NetworkResult};
use crate::networking::client::{Client, ClientConfig};
use std::net::SocketAddr;
use uuid::Uuid;

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
        
        // For official grids, require FirstName LastName format
        if self.grid.is_official() && !self.username.contains(' ') {
            return Err("Username must be in format 'FirstName LastName' for SecondLife grids".to_string());
        }
        
        Ok(())
    }

    /// Split username into first and last name
    pub fn split_name(&self) -> (String, String) {
        let parts: Vec<&str> = self.username.splitn(2, ' ').collect();
        match parts.as_slice() {
            [first] => (first.to_string(), "Resident".to_string()),
            [first, last] => (first.to_string(), last.to_string()),
            _ => ("Unknown".to_string(), "User".to_string()),
        }
    }
}

pub struct AuthenticationService {
    session_manager: SessionManager,
    xmlrpc_client: XmlRpcClient,
}

impl AuthenticationService {
    pub fn new() -> Self {
        Self {
            session_manager: SessionManager::new(),
            xmlrpc_client: XmlRpcClient::new(),
        }
    }
    
    pub async fn login(&mut self, credentials: LoginCredentials) -> NetworkResult<Client> {
        // Validate credentials
        credentials.validate()
            .map_err(|e| NetworkError::AuthenticationFailed { reason: e })?;
        
        // Step 1: Authenticate with login server
        let login_response = self.authenticate_with_login_server(&credentials).await?;
        
        if !login_response.success {
            let reason = login_response.reason
                .or(login_response.message)
                .unwrap_or_else(|| "Login failed".to_string());
            return Err(NetworkError::AuthenticationFailed { reason });
        }
        
        // Step 2: Create session info
        let simulator_address = login_response.simulator_address()
            .map_err(|e| NetworkError::AuthenticationFailed { 
                reason: format!("Invalid simulator address: {}", e) 
            })?;

        let session = SessionInfo {
            agent_id: login_response.agent_id,
            session_id: login_response.session_id,
            secure_session_id: login_response.secure_session_id,
            first_name: login_response.first_name,
            last_name: login_response.last_name,
            circuit_code: login_response.circuit_code,
            simulator_address,
            look_at: login_response.look_at,
            start_location: credentials.start_location,
        };
        
        // Step 3: Start session
        self.session_manager.start_session(session.clone());
        
        // Step 4: Create networking client
        let config = ClientConfig {
            agent_id: session.agent_id,
            session_id: session.session_id,
            ..Default::default()
        };
        
        let client = Client::new(config).await?;
        
        // Step 5: Connect to simulator
        client.connect(session.simulator_address, session.circuit_code).await?;
        
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
    
    async fn authenticate_with_login_server(&self, credentials: &LoginCredentials) -> NetworkResult<super::xmlrpc::LoginResponse> {
        let (first_name, last_name) = credentials.split_name();
        
        let params = LoginParameters::new(&first_name, &last_name, &credentials.password);
        
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