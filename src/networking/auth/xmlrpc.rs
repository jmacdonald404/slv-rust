use anyhow::{Context, Result};
use reqwest::Client;
use roxmltree;
use std::net::SocketAddr;
use uuid::Uuid;
use crate::utils::math::{Vector3, RegionHandle, parsing as math_parsing};
use std::time::Duration;
use tokio::time::sleep;
use super::types::*;

/// XML-RPC client for SecondLife login servers
pub struct XmlRpcClient {
    client: Client,
}

impl XmlRpcClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .user_agent("slv-rust/0.3.0")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Send XML-RPC request to SecondLife login server
    pub async fn login_to_simulator(&self, url: &str, params: LoginParameters) -> Result<LoginResponse> {
        let xml_request = self.build_login_request(&params)?;
        
        tracing::info!("Sending XML-RPC login request to {}", url);
        tracing::debug!("XML-RPC Request Body:\n{}", xml_request);
        
        let response = self.client
            .post(url)
            .header("Content-Type", "text/xml")
            .body(xml_request)
            .send()
            .await
            .context("Failed to send login request")?;

        let status = response.status();
        let xml_body = response.text().await
            .context("Failed to read login response")?;

        tracing::debug!("XML-RPC Response Body:\n{}", xml_body);

        if !status.is_success() {
            anyhow::bail!("Login request failed with status: {}. Response: {}", status, xml_body);
        }

        self.parse_login_response(&xml_body)
    }

    fn build_login_request(&self, params: &LoginParameters) -> Result<String> {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\"?>\n");
        xml.push_str("<methodCall>\n");
        xml.push_str("  <methodName>login_to_simulator</methodName>\n");
        xml.push_str("  <params>\n");
        xml.push_str("    <param>\n");
        xml.push_str("      <value>\n");
        xml.push_str("        <struct>\n");

        // Add all parameters as struct members
        self.add_xml_member(&mut xml, "first", &params.first_name);
        self.add_xml_member(&mut xml, "last", &params.last_name);
        self.add_xml_member(&mut xml, "passwd", &params.password_hash);
        self.add_xml_member(&mut xml, "start", &params.start_location);
        self.add_xml_member(&mut xml, "channel", &params.channel);
        self.add_xml_member(&mut xml, "version", &params.version);
        self.add_xml_member(&mut xml, "platform", &params.platform);
        self.add_xml_member(&mut xml, "mac", &params.mac_address);
        self.add_xml_member(&mut xml, "id0", &params.machine_id);
        self.add_xml_member(&mut xml, "agree_to_tos", &params.agree_to_tos.to_string());
        self.add_xml_member(&mut xml, "read_critical", &params.read_critical.to_string());
        self.add_xml_member(&mut xml, "viewer_digest", &params.viewer_digest);

        // Add MFA parameters if present
        if let Some(ref token) = params.mfa_token {
            self.add_xml_member(&mut xml, "token", token);
        }
        if let Some(ref hash) = params.mfa_hash {
            self.add_xml_member(&mut xml, "mfa_hash", hash);
        }

        // Add options array
        xml.push_str("          <member>\n");
        xml.push_str("            <name>options</name>\n");
        xml.push_str("            <value>\n");
        xml.push_str("              <array>\n");
        xml.push_str("                <data>\n");
        for option in &params.options {
            xml.push_str(&format!("                  <value><string>{}</string></value>\n", option));
        }
        xml.push_str("                </data>\n");
        xml.push_str("              </array>\n");
        xml.push_str("            </value>\n");
        xml.push_str("          </member>\n");

        xml.push_str("        </struct>\n");
        xml.push_str("      </value>\n");
        xml.push_str("    </param>\n");
        xml.push_str("  </params>\n");
        xml.push_str("</methodCall>\n");

        Ok(xml)
    }

    fn add_xml_member(&self, xml: &mut String, name: &str, value: &str) {
        xml.push_str(&format!(
            "          <member>\n            <name>{}</name>\n            <value><string>{}</string></value>\n          </member>\n",
            name, value
        ));
    }

    fn parse_login_response(&self, xml: &str) -> Result<LoginResponse> {
        use roxmltree::Document;

        let doc = Document::parse(xml)
            .context("Failed to parse XML response")?;

        // Navigate to the response struct
        let root = doc.root_element();
        
        // The root element should be methodResponse
        let method_response = if root.tag_name().name() == "methodResponse" {
            root
        } else {
            // Look for methodResponse as a child
            root.children()
                .find(|n| n.tag_name().name() == "methodResponse")
                .context("No methodResponse found")?
        };

        // Check for fault
        if let Some(_fault) = method_response
            .children()
            .find(|n| n.tag_name().name() == "fault") {
            anyhow::bail!("Login fault response received");
        }

        let params = method_response
            .children()
            .find(|n| n.tag_name().name() == "params")
            .context("No params found in response")?;

        let param = params
            .children()
            .find(|n| n.tag_name().name() == "param")
            .context("No param found")?;

        let value = param
            .children()
            .find(|n| n.tag_name().name() == "value")
            .context("No value found")?;

        let struct_elem = value
            .children()
            .find(|n| n.tag_name().name() == "struct")
            .context("No struct found in response")?;

        let mut response = LoginResponse::default();
        
        for member in struct_elem.children().filter(|n| n.tag_name().name() == "member") {
            let name_elem = member
                .children()
                .find(|n| n.tag_name().name() == "name")
                .and_then(|n| n.text());

            let value_elem = member
                .children()
                .find(|n| n.tag_name().name() == "value");

            if let (Some(name), Some(value_node)) = (name_elem, value_elem) {
                // Check if this is a complex field that needs special parsing
                let value_text = if matches!(name, "home_info" | "inventory_root" | "inventory_skeleton" | 
                                              "buddy_list" | "login_flags" | "premium_packages" | 
                                              "account_level_benefits") {
                    self.parse_complex_field(value_node, name)?
                } else {
                    self.extract_value_text(value_node)
                };
                
                self.set_response_field(&mut response, name, &value_text)?;
            }
        }

        Ok(response)
    }

    fn extract_value_text(&self, value_node: roxmltree::Node) -> String {
        // Try to find string, boolean, or other value types
        if let Some(string_node) = value_node.children().find(|n| n.tag_name().name() == "string") {
            string_node.text().unwrap_or("").to_string()
        } else if let Some(boolean_node) = value_node.children().find(|n| n.tag_name().name() == "boolean") {
            boolean_node.text().unwrap_or("0").to_string()
        } else if let Some(int_node) = value_node.children().find(|n| n.tag_name().name() == "int") {
            int_node.text().unwrap_or("0").to_string()
        } else if let Some(double_node) = value_node.children().find(|n| n.tag_name().name() == "double") {
            double_node.text().unwrap_or("0.0").to_string()
        } else if let Some(array_node) = value_node.children().find(|n| n.tag_name().name() == "array") {
            // Handle arrays - convert to comma-separated string
            let mut values = Vec::new();
            for data_node in array_node.children().filter(|n| n.tag_name().name() == "data") {
                for value_node in data_node.children().filter(|n| n.tag_name().name() == "value") {
                    let text = self.extract_value_text(value_node);
                    if !text.is_empty() {
                        values.push(text);
                    }
                }
            }
            values.join(",")
        } else if let Some(struct_node) = value_node.children().find(|n| n.tag_name().name() == "struct") {
            // Handle structs - convert to JSON-like format for complex parsing
            let mut pairs = Vec::new();
            for member_node in struct_node.children().filter(|n| n.tag_name().name() == "member") {
                let name_node = member_node.children().find(|n| n.tag_name().name() == "name");
                let value_node = member_node.children().find(|n| n.tag_name().name() == "value");
                
                if let (Some(name), Some(value)) = (name_node, value_node) {
                    let name_text = name.text().unwrap_or("");
                    let value_text = self.extract_value_text(value);
                    pairs.push(format!("\"{}\":\"{}\"", name_text, value_text));
                }
            }
            format!("{{{}}}", pairs.join(","))
        } else {
            value_node.text().unwrap_or("").to_string()
        }
    }

    fn set_response_field(&self, response: &mut LoginResponse, name: &str, value: &str) -> Result<()> {
        match name {
            // Core login fields
            "login" => {
                response.success = math_parsing::parse_bool(value)
                    .map_err(|e| anyhow::anyhow!("Invalid login value: {}", e))?;
            }
            "agent_id" => {
                response.agent_id = math_parsing::parse_uuid(value)
                    .map_err(|e| anyhow::anyhow!("Invalid agent_id: {}", e))?;
            }
            "session_id" => {
                response.session_id = math_parsing::parse_uuid(value)
                    .map_err(|e| anyhow::anyhow!("Invalid session_id: {}", e))?;
            }
            "secure_session_id" => {
                response.secure_session_id = math_parsing::parse_uuid(value)
                    .map_err(|e| anyhow::anyhow!("Invalid secure_session_id: {}", e))?;
            }
            "first_name" => {
                // Handle quoted names from Second Life
                let cleaned = value.trim_matches('"');
                response.first_name = cleaned.to_string();
            }
            "last_name" => {
                response.last_name = value.to_string();
            }
            "circuit_code" => {
                response.circuit_code = value.parse()
                    .context("Invalid circuit_code")?;
            }
            "sim_ip" => {
                response.simulator_ip = value.to_string();
            }
            "sim_port" => {
                response.simulator_port = value.parse()
                    .context("Invalid sim_port")?;
            }
            "look_at" => {
                response.look_at = Vector3::parse_sl_format(value)
                    .map_err(|e| anyhow::anyhow!("Invalid look_at: {}", e))?;
            }
            "reason" => {
                response.reason = Some(value.to_string());
            }
            "message" => {
                response.message = Some(value.to_string());
            }
            "seed_capability" => {
                response.seed_capability = Some(value.to_string());
            }

            // Additional fields from Second Life
            "agent_access" => {
                response.agent_access = Some(value.to_string());
            }
            "agent_access_max" => {
                response.agent_access_max = Some(value.to_string());
            }
            "agent_region_access" => {
                response.agent_region_access = Some(value.to_string());
            }
            "agent_appearance_service" => {
                response.agent_appearance_service = Some(value.to_string());
            }
            "agent_flags" => {
                response.agent_flags = Some(value.parse()
                    .context("Invalid agent_flags")?);
            }
            "max_agent_groups" => {
                response.max_agent_groups = Some(value.parse()
                    .context("Invalid max_agent_groups")?);
            }
            "openid_url" => {
                response.openid_url = Some(value.to_string());
            }
            "openid_token" => {
                response.openid_token = Some(value.to_string());
            }
            "cof_version" => {
                response.cof_version = Some(value.parse()
                    .context("Invalid cof_version")?);
            }
            "account_type" => {
                response.account_type = Some(value.to_string());
            }
            "linden_status_code" => {
                response.linden_status_code = Some(value.to_string());
            }
            "max_god_level" => {
                response.max_god_level = Some(value.parse()
                    .context("Invalid max_god_level")?);
            }
            "god_level" => {
                response.god_level = Some(value.parse()
                    .context("Invalid god_level")?);
            }
            "seconds_since_epoch" => {
                response.seconds_since_epoch = Some(value.parse()
                    .context("Invalid seconds_since_epoch")?);
            }
            "start_location" => {
                // Start location can be either a string ("last", "home") or coordinates
                if value.contains(',') || value.contains('<') {
                    // Try to parse as Vector3 coordinates
                    response.start_location = Some(Vector3::parse_sl_format(value)
                        .map_err(|e| anyhow::anyhow!("Invalid start_location coordinates: {}", e))?);
                } else {
                    // Handle string locations by converting to default coordinates
                    let default_pos = match value {
                        "last" => Vector3::new(128.0, 128.0, 0.0), // Default region center
                        "home" => Vector3::new(128.0, 128.0, 0.0), // Default home position
                        _ => Vector3::new(128.0, 128.0, 0.0), // Default fallback
                    };
                    response.start_location = Some(default_pos);
                }
            }
            "home" => {
                response.home = Some(Vector3::parse_sl_format(value)
                    .map_err(|e| anyhow::anyhow!("Invalid home: {}", e))?);
            }
            "region_x" => {
                response.region_x = Some(value.parse()
                    .context("Invalid region_x")?);
            }
            "region_y" => {
                response.region_y = Some(value.parse()
                    .context("Invalid region_y")?);
            }
            "map_server_url" => {
                response.map_server_url = Some(value.to_string());
            }
            "udp_blacklist" => {
                response.udp_blacklist = Some(math_parsing::parse_string_array(value));
            }

            // Complex nested fields (these would need special handling for full parsing)
            "home_info" | "inventory_root" | "inventory_skeleton" | 
            "buddy_list" | "login_flags" | "premium_packages" | 
            "account_level_benefits" => {
                // For now, log these complex fields for future implementation
                tracing::debug!("Complex field {} = {} (needs special parsing)", name, value);
            }

            _ => {
                // Store unknown fields for debugging
                tracing::debug!("Unknown login response field: {} = {}", name, value);
            }
        }
        Ok(())
    }

    /// Parse complex nested structures from XML-RPC response
    fn parse_complex_field(&self, value_node: roxmltree::Node, field_name: &str) -> Result<String> {
        match field_name {
            "home_info" => {
                // Parse home_info structure
                let mut home_info = std::collections::HashMap::new();
                for member in value_node.children().filter(|n| n.tag_name().name() == "member") {
                    let name_elem = member
                        .children()
                        .find(|n| n.tag_name().name() == "name")
                        .and_then(|n| n.text());

                    let value_elem = member
                        .children()
                        .find(|n| n.tag_name().name() == "value");

                    if let (Some(name), Some(value_node)) = (name_elem, value_elem) {
                        let value_text = self.extract_value_text(value_node);
                        home_info.insert(name.to_string(), value_text);
                    }
                }
                
                // Convert to JSON-like format
                let pairs: Vec<String> = home_info
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
                    .collect();
                Ok(format!("{{{}}}", pairs.join(",")))
            }
            "inventory_root" | "inventory_skeleton" => {
                // Parse inventory array
                let mut items = Vec::new();
                for data_node in value_node.children().filter(|n| n.tag_name().name() == "data") {
                    for value_node in data_node.children().filter(|n| n.tag_name().name() == "value") {
                        let item_text = self.extract_value_text(value_node);
                        if !item_text.is_empty() {
                            items.push(item_text);
                        }
                    }
                }
                Ok(format!("[{}]", items.join(",")))
            }
            "buddy_list" => {
                // Parse buddy list array
                let mut buddies = Vec::new();
                for data_node in value_node.children().filter(|n| n.tag_name().name() == "data") {
                    for value_node in data_node.children().filter(|n| n.tag_name().name() == "value") {
                        let buddy_text = self.extract_value_text(value_node);
                        if !buddy_text.is_empty() {
                            buddies.push(buddy_text);
                        }
                    }
                }
                Ok(format!("[{}]", buddies.join(",")))
            }
            "login_flags" => {
                // Parse login flags array
                let mut flags = Vec::new();
                for data_node in value_node.children().filter(|n| n.tag_name().name() == "data") {
                    for value_node in data_node.children().filter(|n| n.tag_name().name() == "value") {
                        let flag_text = self.extract_value_text(value_node);
                        if !flag_text.is_empty() {
                            flags.push(flag_text);
                        }
                    }
                }
                Ok(format!("[{}]", flags.join(",")))
            }
            _ => {
                // Default handling for unknown complex fields
                Ok(self.extract_value_text(value_node))
            }
        }
    }
}

impl Default for XmlRpcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct LoginParameters {
    pub first_name: String,
    pub last_name: String,
    pub password_hash: String,
    pub start_location: String,
    pub channel: String,
    pub version: String,
    pub platform: String,
    pub mac_address: String,
    pub machine_id: String,
    pub agree_to_tos: bool,
    pub read_critical: bool,
    pub viewer_digest: String,
    pub options: Vec<String>,
    pub mfa_token: Option<String>,
    pub mfa_hash: Option<String>,
}

impl LoginParameters {
    pub fn new(first: &str, last: &str, password: &str) -> Self {
        Self {
            first_name: first.to_string(),
            last_name: last.to_string(),
            password_hash: Self::hash_password(password),
            start_location: "last".to_string(),
            channel: "slv-rust".to_string(),
            version: "0.3.0".to_string(),
            platform: Self::get_platform(),
            mac_address: Self::get_mac_address(),
            machine_id: Self::get_machine_id(),
            agree_to_tos: true,
            read_critical: true,
            viewer_digest: "00000000-0000-0000-0000-000000000000".to_string(),
            options: vec![
                "inventory-root".to_string(),
                "inventory-skeleton".to_string(),
                "buddy-list".to_string(),
                "login-flags".to_string(),
            ],
            mfa_token: std::env::var("SL_MFA_TOKEN").ok(),
            mfa_hash: std::env::var("SL_MFA_HASH").ok(),
        }
    }

    fn hash_password(password: &str) -> String {
        // SecondLife only uses first 16 characters of password
        let truncated = password.chars().take(16).collect::<String>();
        let digest = md5::compute(truncated.as_bytes());
        format!("$1${:x}", digest)
    }

    fn get_platform() -> String {
        #[cfg(target_os = "windows")]
        return "win".to_string();
        #[cfg(target_os = "macos")]
        return "mac".to_string();
        #[cfg(target_os = "linux")]
        return "lnx".to_string();
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        return "unk".to_string();
    }

    fn get_mac_address() -> String {
        // Simplified - in production you'd want to get the actual MAC address
        "00:00:00:00:00:00".to_string()
    }

    fn get_machine_id() -> String {
        // Simplified - in production you'd want a unique machine identifier
        let digest = md5::compute(b"slv-rust-machine-id");
        format!("{:x}", digest)
    }
}

// LoginResponse is now defined in types.rs