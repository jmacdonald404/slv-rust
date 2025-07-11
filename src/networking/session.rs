use reqwest::Client;
use serde::{Serialize, Deserialize};
use quick_xml::de::from_str;
use quick_xml::events::Event;
use quick_xml::Reader;
use quick_xml::name::QName;
use crate::ui::proxy::ProxySettings;
use tracing::{info, warn};
use regex::Regex;
use roxmltree::Document;
use std::str::FromStr;

#[derive(Serialize, Debug)]
pub struct LoginRequest {
    pub first: String,
    pub last: String,
    pub password: String,
    pub start: String,
    pub channel: String,
    pub version: String,
    pub platform: String,
    pub platform_string: String,
    pub platform_version: String,
    pub mac: String,
    pub id0: String,
    pub agree_to_tos: i32,
    pub address_size: i32,
    pub extended_errors: i32,
    pub host_id: String,
    pub last_exec_duration: i32,
    pub last_exec_event: i32,
    pub last_exec_session_id: String,
    pub mfa_hash: String,
    pub token: String,
    pub read_critical: i32,
    pub options: Vec<String>,
}

impl LoginRequest {
    pub fn default_options() -> Vec<String> {
        vec![
            "inventory-root", "inventory-skeleton", "inventory-lib-root", "inventory-lib-owner",
            "inventory-skel-lib", "initial-outfit", "gestures", "display_names", "event_categories",
            "event_notifications", "classified_categories", "adult_compliant", "buddy-list",
            "newuser-config", "ui-config", "advanced-mode", "max-agent-groups", "map-server-url",
            "voice-config", "tutorial_setting", "login-flags", "global-textures"
        ].into_iter().map(String::from).collect()
    }
}

#[derive(Debug, Clone)]
pub struct LoginSessionInfo {
    // Required fields from protocol spec and real responses
    pub last_name: String,
    pub first_name: String,
    pub agent_id: String,
    pub session_id: String,
    pub secure_session_id: String,
    pub sim_ip: String,
    pub sim_port: u16,
    pub circuit_code: u32,
    pub region_x: i32,
    pub region_y: i32,
    pub look_at: String,
    pub start_location: String,
    pub seconds_since_epoch: i64,
    pub message: String,
    pub inventory_host: String,
    pub seed_capability: String,
    pub agent_access: String,
    pub login: String,
    // New/optional fields
    pub account_type: Option<String>,
    pub linden_status_code: Option<String>,
    pub agent_flags: Option<i32>,
    pub max_god_level: Option<i32>,
    pub god_level: Option<i32>,
    pub inventory_root: Option<String>,
    pub buddy_list: Option<Vec<String>>,
    // TODO: Add more fields as needed (inventory-skeleton, gestures, event_categories, etc.)
}

fn build_login_xml(req: &LoginRequest) -> String {
    let options_xml: String = req.options.iter()
        .map(|opt| format!("<value><string>{}</string></value>", opt))
        .collect::<Vec<_>>()
        .join("\n");

    format!(r#"<?xml version="1.0" ?>
<methodCall>
  <methodName>login_to_simulator</methodName>
  <params>
    <param>
      <value>
        <struct>
          <member><name>address_size</name><value><int>{address_size}</int></value></member>
          <member><name>agree_to_tos</name><value><int>{agree_to_tos}</int></value></member>
          <member><name>channel</name><value><string>{channel}</string></value></member>
          <member><name>extended_errors</name><value><int>{extended_errors}</int></value></member>
          <member><name>first</name><value><string>{first}</string></value></member>
          <member><name>host_id</name><value><string>{host_id}</string></value></member>
          <member><name>id0</name><value><string>{id0}</string></value></member>
          <member><name>last</name><value><string>{last}</string></value></member>
          <member><name>last_exec_duration</name><value><int>{last_exec_duration}</int></value></member>
          <member><name>last_exec_event</name><value><int>{last_exec_event}</int></value></member>
          <member><name>last_exec_session_id</name><value><string>{last_exec_session_id}</string></value></member>
          <member><name>mac</name><value><string>{mac}</string></value></member>
          <member><name>mfa_hash</name><value><string>{mfa_hash}</string></value></member>
          <member><name>options</name>
            <value>
              <array>
                <data>
                  {options_xml}
                </data>
              </array>
            </value>
          </member>
          <member><name>passwd</name><value><string>{password}</string></value></member>
          <member><name>platform</name><value><string>{platform}</string></value></member>
          <member><name>platform_string</name><value><string>{platform_string}</string></value></member>
          <member><name>platform_version</name><value><string>{platform_version}</string></value></member>
          <member><name>read_critical</name><value><int>{read_critical}</int></value></member>
          <member><name>start</name><value><string>{start}</string></value></member>
          <member><name>token</name><value><string>{token}</string></value></member>
          <member><name>version</name><value><string>{version}</string></value></member>
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#,
        address_size = req.address_size,
        agree_to_tos = req.agree_to_tos,
        channel = req.channel,
        extended_errors = req.extended_errors,
        first = req.first,
        host_id = req.host_id,
        id0 = req.id0,
        last = req.last,
        last_exec_duration = req.last_exec_duration,
        last_exec_event = req.last_exec_event,
        last_exec_session_id = req.last_exec_session_id,
        mac = req.mac,
        mfa_hash = req.mfa_hash,
        options_xml = options_xml,
        password = req.password,
        platform = req.platform,
        platform_string = req.platform_string,
        platform_version = req.platform_version,
        read_critical = req.read_critical,
        start = req.start,
        token = req.token,
        version = req.version,
    )
}

pub async fn login_to_secondlife(grid_uri: &str, req: &LoginRequest, proxy_settings: Option<&ProxySettings>) -> Result<LoginSessionInfo, String> {
    info!("[LOGIN] login_to_secondlife called. proxy_settings: {:?}", proxy_settings);
    let xml_body = build_login_xml(req);
    eprintln!("[LOGIN XML BODY]\n{}", xml_body);
    let mut client_builder = reqwest::Client::builder();
    if let Some(proxy) = proxy_settings {
        if proxy.enabled {
            let proxy_url = format!("http://{}:{}", proxy.http_host, proxy.http_port);
            info!("[HTTP PROXY] Using HTTP proxy: {}", proxy_url);
            client_builder = client_builder.proxy(reqwest::Proxy::http(&proxy_url).map_err(|e| format!("Proxy URL error: {e}"))?);
            if proxy.disable_cert_validation {
                info!("[HTTP PROXY] Certificate validation is DISABLED for HTTPS requests");
                client_builder = client_builder.danger_accept_invalid_certs(true);
            } else {
                info!("[HTTP PROXY] Certificate validation is ENABLED for HTTPS requests");
            }
        } else {
            info!("[HTTP PROXY] Proxy is DISABLED; direct connection will be used");
        }
    } else {
        info!("[HTTP PROXY] No proxy settings provided; direct connection will be used");
    }
    let client = client_builder.build().map_err(|e| format!("HTTP client build error: {e}"))?;
    let res = client
        .post(grid_uri)
        .header("Content-Type", "text/xml")
        .body(xml_body)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;
    let status = res.status();
    let text = res.text().await.map_err(|e| format!("HTTP error: {e}"))?;
    // Remove debug file output
    eprintln!("[LOGIN RESPONSE] Raw body length: {}", text.len());
    // Filter out large inventory-skel-lib and inventory-skeleton sections from debug print
    let re = Regex::new(r"(?s)<member>\s*<name>(inventory-skel-lib|inventory-skeleton)</name>\s*<value>\s*<array>.*?</array>\s*</value>\s*</member>").unwrap();
    let filtered_text = re.replace_all(&text, |caps: &regex::Captures| {
        format!("<member><name>{}</name><value><array>[...omitted...]</array></value></member>", &caps[1])
    });
    // Only print the filtered [LOGIN RESPONSE] log
    eprintln!("[LOGIN RESPONSE] HTTP status: {}", status);
    eprintln!("[LOGIN RESPONSE] Raw body:\n{}", filtered_text);
    match parse_login_response(&text) {
        Ok(info) => Ok(info),
        Err(e) => {
            // Only return the error, do not print it here
            Err(format!("Failed to parse login response: {e}"))
        }
    }
}

fn parse_login_response(xml: &str) -> Result<LoginSessionInfo, String> {
    use roxmltree::Document;
    use std::str::FromStr;
    let doc = Document::parse(xml).map_err(|e| format!("XML parse error: {e}"))?;
    // Find the first <struct> node under <methodResponse>
    let struct_node = doc.descendants().find(|n| n.has_tag_name("struct")).ok_or("No <struct> found in login response")?;
    let mut get_field = |field: &str| -> Option<String> {
        for member in struct_node.children().filter(|n| n.has_tag_name("member")) {
            let name = member.children().find(|n| n.has_tag_name("name")).and_then(|n| n.text()).unwrap_or("");
            if name == field {
                // Try to get the value as text from <string>, <int>, or directly from <value>
                let value_node = member.children().find(|n| n.has_tag_name("value"))?;
                if let Some(s) = value_node.children().find(|n| n.has_tag_name("string")).and_then(|n| n.text()) {
                    return Some(s.trim_matches('"').to_string());
                } else if let Some(i) = value_node.children().find(|n| n.has_tag_name("int")).and_then(|n| n.text()) {
                    return Some(i.to_string());
                } else if let Some(t) = value_node.text() {
                    return Some(t.trim_matches('"').to_string());
                }
            }
        }
        None
    };
    Ok(LoginSessionInfo {
        last_name: get_field("last_name").or_else(|| get_field("last")).ok_or("missing last_name")?,
        first_name: get_field("first_name").or_else(|| get_field("first")).ok_or("missing first_name")?,
        agent_id: get_field("agent_id").ok_or("missing agent_id")?,
        session_id: get_field("session_id").ok_or("missing session_id")?,
        secure_session_id: get_field("secure_session_id").ok_or("missing secure_session_id")?,
        sim_ip: get_field("sim_ip").ok_or("missing sim_ip")?,
        sim_port: get_field("sim_port").and_then(|v| v.trim().parse::<u16>().ok()).ok_or("missing sim_port")?,
        circuit_code: get_field("circuit_code").and_then(|v| v.trim().parse::<u32>().ok()).ok_or("missing circuit_code")?,
        region_x: get_field("region_x").and_then(|v| v.trim().parse::<i32>().ok()).ok_or("missing region_x")?,
        region_y: get_field("region_y").and_then(|v| v.trim().parse::<i32>().ok()).ok_or("missing region_y")?,
        look_at: get_field("look_at").ok_or("missing look_at")?,
        start_location: get_field("start_location").or_else(|| get_field("start")).ok_or("missing start_location")?,
        seconds_since_epoch: get_field("seconds_since_epoch").and_then(|v| v.trim().parse::<i64>().ok()).ok_or("missing seconds_since_epoch")?,
        message: get_field("message").unwrap_or_default(),
        inventory_host: get_field("inventory_host").unwrap_or_default(),
        seed_capability: get_field("seed_capability").unwrap_or_default(),
        agent_access: get_field("agent_access").unwrap_or_default(),
        login: get_field("login").unwrap_or_default(),
        account_type: get_field("account_type"),
        linden_status_code: get_field("Linden_Status_Code"),
        agent_flags: get_field("agent_flags").and_then(|v| v.trim().parse::<i32>().ok()),
        max_god_level: get_field("max_god_level").and_then(|v| v.trim().parse::<i32>().ok()),
        god_level: get_field("god_level").and_then(|v| v.trim().parse::<i32>().ok()),
        inventory_root: get_field("inventory-root"),
        buddy_list: None, // TODO: parse buddy-list if needed
    })
}

pub async fn fetch_tos_html(
    tos_id: &str,
    udp_port: Option<u16>,
    proxy_settings: Option<&ProxySettings>,
) -> Result<String, String> {
    let url = format!("https://secondlife.com/app/tos/tos.php?id={}", tos_id);

    let mut client_builder = reqwest::Client::builder();
    if let Some(proxy) = proxy_settings {
        if proxy.enabled {
            let proxy_url = format!("http://{}:{}", proxy.http_host, proxy.http_port);
            client_builder = client_builder.proxy(reqwest::Proxy::http(&proxy_url).map_err(|e| format!("Proxy URL error: {e}"))?);
            if proxy.disable_cert_validation {
                client_builder = client_builder.danger_accept_invalid_certs(true);
            }
        }
    }
    let client = client_builder.build().map_err(|e| format!("HTTP client build error: {e}"))?;

    let mut req = client.get(&url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml");
    if let Some(port) = udp_port {
        req = req.header("X-SecondLife-UDP-Listen-Port", port);
    }

    let resp = req.send().await.map_err(|e| format!("TOS GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("TOS GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("TOS GET failed: HTTP {}", status));
    }
    Ok(text)
}
