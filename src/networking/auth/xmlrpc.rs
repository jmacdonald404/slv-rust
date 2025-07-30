use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

/// XML-RPC client for SecondLife login servers
pub struct XmlRpcClient {
    client: Client,
}

impl XmlRpcClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("slv-rust/0.3.0")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Send XML-RPC request to SecondLife login server
    pub async fn login_to_simulator(&self, url: &str, params: LoginParameters) -> Result<LoginResponse> {
        let xml_request = self.build_login_request(&params)?;
        
        tracing::info!("Sending XML-RPC login request to {}", url);
        
        let response = self.client
            .post(url)
            .header("Content-Type", "text/xml")
            .body(xml_request)
            .send()
            .await
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            anyhow::bail!("Login request failed with status: {}", response.status());
        }

        let xml_body = response.text().await
            .context("Failed to read login response")?;

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
                let value_text = self.extract_value_text(value_node);
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
        } else {
            value_node.text().unwrap_or("").to_string()
        }
    }

    fn set_response_field(&self, response: &mut LoginResponse, name: &str, value: &str) -> Result<()> {
        match name {
            "login" => {
                response.success = value == "true";
            }
            "agent_id" => {
                response.agent_id = Uuid::parse_str(value)
                    .context("Invalid agent_id UUID")?;
            }
            "session_id" => {
                response.session_id = Uuid::parse_str(value)
                    .context("Invalid session_id UUID")?;
            }
            "secure_session_id" => {
                response.secure_session_id = Uuid::parse_str(value)
                    .context("Invalid secure_session_id UUID")?;
            }
            "first_name" => {
                response.first_name = value.to_string();
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
                // Parse look_at array format: r1,0,0 or [1.0, 0.0, 0.0]
                let coords: Result<Vec<f32>, _> = value
                    .trim_start_matches(['r', '['])
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().parse())
                    .collect();
                
                if let Ok(coords) = coords {
                    if coords.len() >= 3 {
                        response.look_at = [coords[0], coords[1], coords[2]];
                    }
                }
            }
            "reason" => {
                response.reason = Some(value.to_string());
            }
            "message" => {
                response.message = Some(value.to_string());
            }
            _ => {
                // Store unknown fields for debugging
                tracing::debug!("Unknown login response field: {} = {}", name, value);
            }
        }
        Ok(())
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

#[derive(Debug, Clone, Default)]
pub struct LoginResponse {
    pub success: bool,
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub secure_session_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub circuit_code: u32,
    pub simulator_ip: String,
    pub simulator_port: u16,
    pub look_at: [f32; 3],
    pub reason: Option<String>,
    pub message: Option<String>,
}

impl LoginResponse {
    pub fn simulator_address(&self) -> Result<SocketAddr> {
        let addr = format!("{}:{}", self.simulator_ip, self.simulator_port);
        addr.parse().context("Invalid simulator address")
    }
}