//! Seed capability handling for Second Life protocol compliance
//! 
//! This module handles the initial "seed" capability request that fetches
//! all available capability URLs from the simulator, matching the official
//! viewer's behavior as documented in hippolog analysis.

use std::collections::HashMap;
use tracing::{info, debug, warn, error};
use reqwest::Client;
use crate::networking::{NetworkError, NetworkResult};
use crate::ui::proxy::ProxySettings;
use std::fs;

/// Complete list of capabilities requested by the official Second Life viewer
/// This list is derived from hippolog analysis of official viewer behavior
pub const OFFICIAL_VIEWER_CAPABILITIES: &[&str] = &[
    "AbuseCategories",
    "AcceptFriendship", 
    "AcceptGroupInvite",
    "AgentPreferences",
    "AgentProfile",
    "AgentState",
    "AttachmentResources",
    "AvatarPickerSearch",
    "AvatarRenderInfo",
    "CharacterProperties",
    "ChatSessionRequest",
    "CopyInventoryFromNotecard",
    "CreateInventoryCategory",
    "DeclineFriendship",
    "DeclineGroupInvite",
    "DispatchRegionInfo",
    "DirectDelivery",
    "EnvironmentSettings",
    "EstateAccess",
    "EstateChangeInfo",
    "EventQueueGet",
    "ExtEnvironment",
    "FetchLib2",
    "FetchLibDescendents2",
    "FetchInventory2",
    "FetchInventoryDescendents2",
    "IncrementCOFVersion",
    "RequestTaskInventory",
    "InventoryAPIv3",
    "LibraryAPIv3",
    "InterestList",
    "InventoryThumbnailUpload",
    "GetDisplayNames",
    "GetExperiences",
    "AgentExperiences",
    "FindExperienceByName",
    "GetExperienceInfo",
    "GetAdminExperiences",
    "GetCreatorExperiences",
    "ExperiencePreferences",
    "GroupExperiences",
    "UpdateExperience",
    "IsExperienceAdmin",
    "IsExperienceContributor",
    "RegionExperiences",
    "ExperienceQuery",
    "GetMetadata",
    "GetObjectCost",
    "GetObjectPhysicsData",
    "GroupAPIv1",
    "GroupMemberData",
    "GroupProposalBallot",
    "HomeLocation",
    "LandResources",
    "LSLSyntax",
    "MapLayer",
    "MapLayerGod",
    "MeshUploadFlag",
    "ModifyMaterialParams",
    "ModifyRegion",
    "NavMeshGenerationStatus",
    "NewFileAgentInventory",
    "ObjectAnimation",
    "ObjectMedia",
    "ObjectMediaNavigate",
    "ObjectNavMeshProperties",
    "ParcelPropertiesUpdate",
    "ParcelVoiceInfoRequest",
    "ProductInfoRequest",
    "ProvisionVoiceAccountRequest",
    "VoiceSignalingRequest",
    "ReadOfflineMsgs",
    "RegionObjects",
    "RegionSchedule",
    "RemoteParcelRequest",
    "RenderMaterials",
    "RequestTextureDownload",
    "ResourceCostSelected",
    "RetrieveNavMeshSrc",
    "SearchStatRequest",
    "SearchStatTracking",
    "SendPostcard",
    "SendUserReport",
    "SendUserReportWithScreenshot",
    "ServerReleaseNotes",
    "SetDisplayName",
    "SimConsoleAsync",
    "SimulatorFeatures",
    "StartGroupProposal",
    "TerrainNavMeshProperties",
    "TextureStats",
    "UntrustedSimulatorMessage",
    "UpdateAgentInformation",
    "UpdateAgentLanguage",
    "UpdateAvatarAppearance",
    "UpdateGestureAgentInventory",
    "UpdateGestureTaskInventory",
    "UpdateNotecardAgentInventory",
    "UpdateNotecardTaskInventory",
    "UpdateScriptAgent",
    "UpdateScriptTask",
    "UpdateSettingsAgentInventory",
    "UpdateSettingsTaskInventory",
    "UploadAgentProfileImage",
    "UpdateMaterialAgentInventory",
    "UpdateMaterialTaskInventory",
    "UploadBakedTexture",
    "UserInfo",
    "ViewerAsset",
    "ViewerBenefits",
    "ViewerMetrics",
    "ViewerStartAuction",
    "ViewerStats",
];

/// Seed capability client that handles fetching all capabilities from a seed URL
pub struct SeedCapabilityClient {
    client: Client,
}

impl SeedCapabilityClient {
    /// Create new seed capability client
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create new seed capability client with proxy configuration
    pub fn new_with_proxy(proxy_settings: Option<&ProxySettings>) -> Self {
        let client = Self::build_proxied_client(proxy_settings);
        Self { client }
    }

    /// Build a reqwest client with proxy settings if enabled (like main branch)
    fn build_proxied_client(proxy_settings: Option<&ProxySettings>) -> Client {
        let mut builder = Client::builder();
        
        if let Some(proxy_cfg) = proxy_settings {
            if proxy_cfg.enabled {
                // Always use the proxy when enabled
                let proxy_url = format!("http://{}:{}", proxy_cfg.http_host, proxy_cfg.http_port);
                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    builder = builder.proxy(proxy);
                }
                
                // Add CA certificate for Hippolyzer
                if let Ok(ca_cert) = fs::read("src/assets/CA.pem") {
                    if let Ok(cert) = reqwest::Certificate::from_pem(&ca_cert) {
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

    /// Fetch all capabilities from the seed capability URL
    /// This matches the official viewer's behavior exactly
    pub async fn fetch_capabilities(&self, seed_url: &str) -> NetworkResult<HashMap<String, String>> {
        info!("🌱 SEED CAPABILITY: Fetching capabilities from {}", seed_url);
        info!("🌱 SEED CAPABILITY: Requesting {} capabilities", OFFICIAL_VIEWER_CAPABILITIES.len());
        
        // Generate LLSD XML request body matching official viewer format
        let request_body = self.generate_capability_request_llsd();
        
        debug!("🌱 SEED CAPABILITY: Request body length: {} bytes", request_body.len());
        debug!("🌱 SEED CAPABILITY: Request capabilities: {:?}", OFFICIAL_VIEWER_CAPABILITIES);
        
        // Use reqwest async client
        let response = self.client
            .post(seed_url)
            .header("Content-Type", "application/llsd+xml")
            .header("Accept", "application/llsd+xml")
            .header("Accept-Encoding", "deflate, gzip")
            .header("Connection", "keep-alive")
            .header("Keep-Alive", "300")
            .header("User-Agent", "Second Life Release 7.1.15 (1559633637437)")
            .body(request_body)
            .send()
            .await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to send seed capability request: {}", e) })?;

        let status_code = response.status().as_u16();
        let response_headers: HashMap<String, String> = response.headers()
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
            .collect();
        let response_text = response.text().await
            .map_err(|e| NetworkError::Transport { reason: format!("Failed to read seed capability response: {}", e) })?;

        if status_code < 200 || status_code >= 300 {
            error!("❌ SEED CAPABILITY: Request failed with status {}", status_code);
            return Err(NetworkError::Transport { 
                reason: format!("Seed capability request failed with status: {}", status_code) 
            });
        }

        info!("✅ SEED CAPABILITY: Response received");
        info!("   Status: {}", status_code);
        info!("   Response size: {} bytes", response_text.len());
        
        debug!("🌱 SEED CAPABILITY: Response headers: {:?}", response_headers);
        debug!("🌱 SEED CAPABILITY: Raw response (first 500 chars): {}", 
               response_text.chars().take(500).collect::<String>());

        // Parse the LLSD XML response
        let capabilities = self.parse_seed_response(&response_text)?;
        
        info!("✅ SEED CAPABILITY: Parsed {} capabilities successfully", capabilities.len());
        info!("✅ SEED CAPABILITY: Available capabilities: {:?}", capabilities.keys().collect::<Vec<_>>());
        
        // Log any missing capabilities that we requested but didn't receive
        let received_caps: std::collections::HashSet<&String> = capabilities.keys().collect();
        
        for &requested_cap in OFFICIAL_VIEWER_CAPABILITIES {
            if !received_caps.iter().any(|&cap| cap == requested_cap) {
                warn!("⚠️ SEED CAPABILITY: Requested capability '{}' not provided by server", requested_cap);
            }
        }

        Ok(capabilities)
    }

    /// Generate LLSD XML request body matching official viewer format
    fn generate_capability_request_llsd(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" ?><llsd><array>");
        
        for &capability in OFFICIAL_VIEWER_CAPABILITIES {
            xml.push_str(&format!("<string>{}</string>", capability));
        }
        
        xml.push_str("</array></llsd>");
        xml
    }

    /// Parse the seed capability response LLSD XML
    fn parse_seed_response(&self, xml_text: &str) -> NetworkResult<HashMap<String, String>> {
        use quick_xml::Reader;
        use quick_xml::events::Event;

        let mut reader = Reader::from_str(xml_text);
        reader.trim_text(true);

        let mut capabilities = HashMap::new();
        let mut buf = Vec::new();
        let mut current_key: Option<String> = None;
        let mut current_value: Option<String> = None;
        let mut in_map = false;
        let mut in_key = false;
        let mut in_string = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    match e.name().as_ref() {
                        b"llsd" => {
                            debug!("🌱 SEED CAPABILITY PARSER: Found LLSD root");
                        }
                        b"map" => {
                            in_map = true;
                            debug!("🌱 SEED CAPABILITY PARSER: Entering map");
                        }
                        b"key" => {
                            in_key = true;
                            current_key = None;
                        }
                        b"string" => {
                            in_string = true;
                            current_value = None;
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.name().as_ref() {
                        b"llsd" => {
                            debug!("🌱 SEED CAPABILITY PARSER: Finished parsing LLSD");
                            break;
                        }
                        b"map" => {
                            in_map = false;
                            debug!("🌱 SEED CAPABILITY PARSER: Exiting map with {} capabilities", capabilities.len());
                        }
                        b"key" => {
                            in_key = false;
                        }
                        b"string" => {
                            in_string = false;
                            
                            // Store the key-value pair when we have both
                            if let (Some(key), Some(value)) = (current_key.take(), current_value.take()) {
                                debug!("🌱 SEED CAPABILITY PARSER: Capability: {} -> {}", key, value);
                                capabilities.insert(key, value);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().map_err(|e| NetworkError::Transport { 
                        reason: format!("Failed to unescape XML text: {}", e) 
                    })?.into_owned();
                    
                    if in_key {
                        current_key = Some(text.trim().to_string());
                    } else if in_string && in_map {
                        current_value = Some(text.trim().to_string());
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    error!("❌ SEED CAPABILITY PARSER: XML parsing error: {}", e);
                    return Err(NetworkError::Transport { 
                        reason: format!("Failed to parse seed capability XML: {}", e) 
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        if capabilities.is_empty() {
            warn!("⚠️ SEED CAPABILITY PARSER: No capabilities parsed from response");
            warn!("⚠️ SEED CAPABILITY PARSER: Response text: {}", xml_text);
        }

        Ok(capabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_request_generation() {
        let client = SeedCapabilityClient::new(reqwest::Client::new());
        let request = client.generate_capability_request_llsd();
        
        // Should contain all official capabilities
        for &cap in OFFICIAL_VIEWER_CAPABILITIES {
            assert!(request.contains(&format!("<string>{}</string>", cap)));
        }
        
        // Should be valid LLSD XML structure
        assert!(request.starts_with("<?xml version=\"1.0\" ?><llsd><array>"));
        assert!(request.ends_with("</array></llsd>"));
    }

    #[test]
    fn test_capability_count() {
        // Ensure we're requesting the same number of capabilities as official viewer
        assert_eq!(OFFICIAL_VIEWER_CAPABILITIES.len(), 117, 
                  "Capability count should match official viewer (117 capabilities)");
    }
}