//! HTTP-based Capabilities system for Second Life
//! 
//! The Capabilities system provides HTTP endpoints for modern SL functionality
//! including inventory operations, asset transfers, and other services that
//! operate outside the UDP packet protocol per netplan.md.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use std::sync::Arc;
use uuid::Uuid;
use ureq::Agent;
use std::io::Read;

pub mod client;
pub mod handlers;

/// Capability URL and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// The HTTP URL for this capability
    pub url: String,
    /// Capability name/identifier
    pub name: String,
    /// Optional expiration time
    pub expires: Option<std::time::SystemTime>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Capabilities manager that handles HTTP-based SL services
#[derive(Debug)]
pub struct CapabilitiesManager {
    /// Map of capability name to capability info
    capabilities: Arc<RwLock<HashMap<String, Capability>>>,
    /// HTTP client for making capability requests
    http_agent: ureq::Agent,
    /// Session information
    session_info: SessionInfo,
}

/// Session information for capability requests
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub region_id: Uuid,
}

impl CapabilitiesManager {
    /// Create a new capabilities manager
    pub fn new(session_info: SessionInfo) -> Self {
        let http_agent = ureq::Agent::new_with_defaults();

        Self {
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            http_agent,
            session_info,
        }
    }

    /// Register capabilities from a seed capability response
    pub async fn register_capabilities(&self, capabilities: HashMap<String, String>) -> Result<(), CapabilityError> {
        let mut caps = self.capabilities.write().await;
        
        info!("📡 Registering {} capabilities", capabilities.len());
        
        for (name, url) in capabilities {
            debug!("📋 Capability: {} -> {}", name, url);
            
            let capability = Capability {
                url,
                name: name.clone(),
                expires: None, // TODO: Parse expiration from response headers
                metadata: HashMap::new(),
            };
            
            caps.insert(name, capability);
        }
        
        info!("✅ Registered {} capabilities", caps.len());
        Ok(())
    }

    /// Get a capability by name
    pub async fn get_capability(&self, name: &str) -> Option<Capability> {
        let caps = self.capabilities.read().await;
        caps.get(name).cloned()
    }

    /// Make an HTTP request to a capability endpoint
    pub async fn capability_request(
        &self,
        capability_name: &str,
        method: CapabilityMethod,
        body: Option<serde_json::Value>,
    ) -> Result<CapabilityResponse, CapabilityError> {
        let capability = self.get_capability(capability_name).await
            .ok_or_else(|| CapabilityError::CapabilityNotFound(capability_name.to_string()))?;

        info!("🌐 CAPABILITY REQUEST: Making {} request", method.as_str());
        info!("   Capability: {}", capability_name);
        info!("   URL: {}", capability.url);
        if body.is_some() {
            info!("   Has request body: true");
        }
        
        let request_start = std::time::Instant::now();
        let agent = self.http_agent.clone();
        let url = capability.url.clone();
        let body_json = body.map(|b| serde_json::to_string(&b)).transpose()
            .map_err(|e| CapabilityError::ParseError(e.to_string()))?;
        
        // Use spawn_blocking since ureq is synchronous
        let (status, headers_map, response_body) = tokio::task::spawn_blocking(move || -> Result<(u16, HashMap<String, String>, String), ureq::Error> {
            let mut response = match method {
                CapabilityMethod::Get => {
                    let request = agent.get(&url)
                        .header("X-SecondLife-Shard", "Production")
                        .header("User-Agent", "slv-rust/0.3.0");
                    request.call()?
                },
                CapabilityMethod::Post => {
                    let request = agent.post(&url)
                        .header("X-SecondLife-Shard", "Production")
                        .header("User-Agent", "slv-rust/0.3.0")
                        .header("Content-Type", "application/json");
                    if let Some(body_str) = body_json {
                        request.send(&body_str)?
                    } else {
                        request.send("")?
                    }
                },
                CapabilityMethod::Put => {
                    let request = agent.put(&url)
                        .header("X-SecondLife-Shard", "Production")
                        .header("User-Agent", "slv-rust/0.3.0")
                        .header("Content-Type", "application/json");
                    if let Some(body_str) = body_json {
                        request.send(&body_str)?
                    } else {
                        request.send("")?
                    }
                },
                CapabilityMethod::Delete => {
                    let request = agent.delete(&url)
                        .header("X-SecondLife-Shard", "Production")
                        .header("User-Agent", "slv-rust/0.3.0");
                    request.call()?
                }
            };

            let status = response.status();
            let headers_map: HashMap<String, String> = response.headers()
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
                .collect();
            
            let response_body = response.body_mut().read_to_string()?;
            
            Ok((status.into(), headers_map, response_body))
        }).await
        .map_err(|e| CapabilityError::HttpError(e.to_string()))?
        .map_err(|e| CapabilityError::HttpError(e.to_string()))?;

        let response_time = request_start.elapsed();

        info!("🌐 CAPABILITY RESPONSE: Response received");
        info!("   Status: {}", status);
        info!("   Response time: {:?}", response_time);
        info!("   Body size: {} bytes", response_body.len());
        info!("   Capability: {}", capability_name);

        if status >= 200 && status < 300 {
            info!("✅ CAPABILITY RESPONSE: Request successful");
            let json_body = if response_body.is_empty() {
                serde_json::Value::Null
            } else {
                match serde_json::from_str(&response_body) {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        warn!("❌ CAPABILITY RESPONSE ERROR: Failed to parse JSON response");
                        warn!("   Error: {}", e);
                        warn!("   Response body: {}", response_body);
                        return Err(CapabilityError::ParseError(e.to_string()));
                    }
                }
            };

            Ok(CapabilityResponse {
                status,
                headers: headers_map,
                body: json_body,
            })
        } else {
            Err(CapabilityError::HttpStatusError(status, response_body))
        }
    }

    /// List all registered capabilities
    pub async fn list_capabilities(&self) -> Vec<String> {
        let caps = self.capabilities.read().await;
        caps.keys().cloned().collect()
    }
}

/// HTTP methods for capability requests
#[derive(Debug, Clone, Copy)]
pub enum CapabilityMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl CapabilityMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            CapabilityMethod::Get => "GET",
            CapabilityMethod::Post => "POST",
            CapabilityMethod::Put => "PUT",
            CapabilityMethod::Delete => "DELETE",
        }
    }
}

/// Response from a capability HTTP request
#[derive(Debug, Clone)]
pub struct CapabilityResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
}

/// Errors that can occur in the capabilities system
#[derive(Debug, thiserror::Error)]
pub enum CapabilityError {
    #[error("Capability not found: {0}")]
    CapabilityNotFound(String),
    
    #[error("HTTP error: {0}")]
    HttpError(String),
    
    #[error("HTTP status error: {0} - {1}")]
    HttpStatusError(u16, String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Common capability endpoints used by Second Life
pub mod well_known_capabilities {
    /// Inventory operations
    pub const FETCH_INVENTORY: &str = "FetchInventory2";
    pub const FETCH_LIB_ROOT: &str = "FetchLibRoot";
    pub const FETCH_LIB: &str = "FetchLib2";
    
    /// Asset operations
    pub const GET_TEXTURE: &str = "GetTexture";
    pub const GET_MESH: &str = "GetMesh2";
    pub const UPLOAD_BAKED_TEXTURE: &str = "UploadBakedTexture";
    
    /// Avatar and appearance
    pub const GET_DISPLAY_NAMES: &str = "GetDisplayNames";
    pub const AVATAR_RENDER_INFO: &str = "AvatarRenderInfo";
    
    /// Economy and transactions
    pub const ECONOMY: &str = "Economy";
    pub const MONEY_BALANCE_REQUEST: &str = "MoneyBalanceRequest";
    
    /// Group operations
    pub const GROUP_MEMBERSHIP: &str = "GroupMembership";
    pub const GROUP_PROPOSAL_BALLOT: &str = "GroupProposalBallot";
    
    /// Event queue for async notifications
    pub const EVENT_QUEUE_GET: &str = "EventQueueGet";
    
    /// Map tile requests
    pub const MAP_LAYER: &str = "MapLayer";
    pub const MAP_LAYER_GOD: &str = "MapLayerGod";
    
    /// Parcel and land operations
    pub const PARCEL_PROPERTIES_UPDATE: &str = "ParcelPropertiesUpdate";
    pub const LAND_RESOURCES: &str = "LandResources";
    
    /// Search operations
    pub const SEARCH: &str = "Search";
    
    /// Object operations
    pub const OBJECT_MEDIA: &str = "ObjectMedia";
    pub const OBJECT_MEDIA_NAVIGATE: &str = "ObjectMediaNavigate";
    
    /// Voice operations
    pub const PROVISION_VOICE_ACCOUNT_REQUEST: &str = "ProvisionVoiceAccountRequest";
    pub const PARCEL_VOICE_INFO_REQUEST: &str = "ParcelVoiceInfoRequest";
}