use uuid::Uuid;
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::utils::math::Vector3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub secure_session_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub circuit_code: u32,
    pub simulator_address: SocketAddr,
    pub look_at: Vector3,
    pub start_location: String,
    pub seed_capability: Option<String>,
    pub capabilities: Option<HashMap<String, String>>,
}

impl SessionInfo {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

#[derive(Debug, Clone)]
pub struct SessionManager {
    current_session: Option<SessionInfo>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            current_session: None,
        }
    }
    
    pub fn start_session(&mut self, session: SessionInfo) {
        self.current_session = Some(session);
    }
    
    pub fn end_session(&mut self) {
        self.current_session = None;
    }
    
    pub fn current_session(&self) -> Option<&SessionInfo> {
        self.current_session.as_ref()
    }
    
    pub fn is_logged_in(&self) -> bool {
        self.current_session.is_some()
    }
    
    pub fn agent_id(&self) -> Option<Uuid> {
        self.current_session.as_ref().map(|s| s.agent_id)
    }
    
    pub fn session_id(&self) -> Option<Uuid> {
        self.current_session.as_ref().map(|s| s.session_id)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}