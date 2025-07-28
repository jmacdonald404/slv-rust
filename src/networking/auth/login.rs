use super::{Grid, SessionInfo, SessionManager};
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
        
        // Additional validation for username format
        if !self.username.contains(' ') && self.grid != Grid::OpenSimulator("Local OpenSim".to_string()) {
            return Err("Username must be in format 'FirstName LastName'".to_string());
        }
        
        Ok(())
    }
}

pub struct AuthenticationService {
    session_manager: SessionManager,
}

impl AuthenticationService {
    pub fn new() -> Self {
        Self {
            session_manager: SessionManager::new(),
        }
    }
    
    pub async fn login(&mut self, credentials: LoginCredentials) -> NetworkResult<Client> {
        // Validate credentials
        credentials.validate()
            .map_err(|e| NetworkError::AuthenticationFailed { reason: e })?;
        
        // Step 1: Authenticate with login server
        let login_response = self.authenticate_with_login_server(&credentials).await?;
        
        // Step 2: Create session info
        let session = SessionInfo {
            agent_id: login_response.agent_id,
            session_id: login_response.session_id,
            secure_session_id: login_response.secure_session_id,
            first_name: login_response.first_name,
            last_name: login_response.last_name,
            circuit_code: login_response.circuit_code,
            simulator_address: login_response.simulator_address,
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
    
    async fn authenticate_with_login_server(&self, credentials: &LoginCredentials) -> NetworkResult<LoginResponse> {
        // Simulate authentication delay
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        // For now, simulate successful login with fake data
        // In a real implementation, this would make HTTP request to login server
        Ok(LoginResponse {
            agent_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            secure_session_id: Uuid::new_v4(),
            first_name: credentials.username.split_whitespace().next().unwrap_or("Test").to_string(),
            last_name: credentials.username.split_whitespace().nth(1).unwrap_or("User").to_string(),
            circuit_code: 12345,
            simulator_address: "127.0.0.1:9000".parse().unwrap(),
            look_at: [1.0, 0.0, 0.0],
        })
    }
}

impl Default for AuthenticationService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct LoginResponse {
    agent_id: Uuid,
    session_id: Uuid,
    secure_session_id: Uuid,
    first_name: String,
    last_name: String,
    circuit_code: u32,
    simulator_address: SocketAddr,
    look_at: [f32; 3],
}