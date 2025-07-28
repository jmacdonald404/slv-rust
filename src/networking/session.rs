use reqwest::Client;
use reqwest::Url;
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
use std::collections::HashMap;
use md5;
use std::fs;
use reqwest::Certificate;
use std::sync::Arc;
use tokio::task::JoinHandle;

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
// Stores the capability name->URL map returned from the capabilities POST.
// TODO: If we use these capabilities a lot, consider adding helper methods for common lookups.
pub struct Capabilities {
    pub map: HashMap<String, String>,
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
    pub capabilities: Option<Capabilities>,
    // TODO: Add more fields as needed (inventory-skeleton, gestures, event_categories, etc.)
    pub session_cookie: Option<String>, // Stores agni_sl_session_id for later use
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    NotStarted,
    SentUseCircuitCode,
    SentCompleteAgentMovement,
    ReceivedRegionHandshake,
    SentRegionHandshakeReply,
    SentAgentThrottle,
    SentFirstAgentUpdate,
    HandshakeComplete,
}

// fn build_login_xml(req: &LoginRequest) -> String {
//     let options_xml: String = req.options.iter()
//         .map(|opt| format!("<value><string>{}</string></value>", opt))
//         .collect::<Vec<_>>()
//         .join("\n");

//     format!(r#"<?xml version="1.0" ?>
// <methodCall>
//   <methodName>login_to_simulator</methodName>
//   <params>
//     <param>
//       <value>
//         <struct>
//           <member><name>address_size</name><value><int>{address_size}</int></value></member>
//           <member><name>agree_to_tos</name><value><int>{agree_to_tos}</int></value></member>
//           <member><name>channel</name><value><string>{channel}</string></value></member>
//           <member><name>extended_errors</name><value><int>{extended_errors}</int></value></member>
//           <member><name>first</name><value><string>{first}</string></value></member>
//           <member><name>host_id</name><value><string>{host_id}</string></value></member>
//           <member><name>id0</name><value><string>{id0}</string></value></member>
//           <member><name>last</name><value><string>{last}</string></value></member>
//           <member><name>last_exec_duration</name><value><int>{last_exec_duration}</int></value></member>
//           <member><name>last_exec_event</name><value><int>{last_exec_event}</int></value></member>
//           <member><name>last_exec_session_id</name><value><string>{last_exec_session_id}</string></value></member>
//           <member><name>mac</name><value><string>{mac}</string></value></member>
//           <member><name>mfa_hash</name><value><string>{mfa_hash}</string></value></member>
//           <member><name>options</name>
//             <value>
//               <array>
//                 <data>
//                   {options_xml}
//                 </data>
//               </array>
//             </value>
//           </member>
//           <member><name>passwd</name><value><string>{password}</string></value></member>
//           <member><name>platform</name><value><string>{platform}</string></value></member>
//           <member><name>platform_string</name><value><string>{platform_string}</string></value></member>
//           <member><name>platform_version</name><value><string>{platform_version}</string></value></member>
//           <member><name>read_critical</name><value><int>{read_critical}</int></value></member>
//           <member><name>start</name><value><string>{start}</string></value></member>
//           <member><name>token</name><value><string>{token}</string></value></member>
//           <member><name>version</name><value><string>{version}</string></value></member>
//         </struct>
//       </value>
//     </param>
//   </params>
// </methodCall>"#,
//         address_size = req.address_size,
//         agree_to_tos = req.agree_to_tos,
//         channel = req.channel,
//         extended_errors = req.extended_errors,
//         first = req.first,
//         host_id = req.host_id,
//         id0 = req.id0,
//         last = req.last,
//         last_exec_duration = req.last_exec_duration,
//         last_exec_event = req.last_exec_event,
//         last_exec_session_id = req.last_exec_session_id,
//         mac = req.mac,
//         mfa_hash = req.mfa_hash,
//         options_xml = options_xml,
//         password = req.password,
//         platform = req.platform,
//         platform_string = req.platform_string,
//         platform_version = req.platform_version,
//         read_critical = req.read_critical,
//         start = req.start,
//         token = req.token,
//         version = req.version,
//     )
// }

// Helper to always build a reqwest::Client with proxy settings if enabled
fn build_proxied_client(_proxy_settings: Option<&crate::ui::proxy::ProxySettings>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder();
    // Always use the proxy for this test
    builder = builder.proxy(reqwest::Proxy::all("http://127.0.0.1:9062").unwrap());
    let ca_cert_path = std::env::var("CARGO_MANIFEST_DIR")
        .map(|dir| format!("{}/src/assets/CA.pem", dir))
        .unwrap_or_else(|_| "src/assets/CA.pem".to_string());
    let ca_cert = fs::read(&ca_cert_path).expect("Failed to read Hippolyzer CA cert");
    let ca_cert = Certificate::from_pem(&ca_cert).expect("Invalid CA cert");
    builder = builder.add_root_certificate(ca_cert);
    builder.build().unwrap()
}

// Add this helper for logging
fn log_http_request(method: &str, url: &str, proxy_settings: Option<&crate::ui::proxy::ProxySettings>, headers: &reqwest::header::HeaderMap, body: Option<&str>) {
    println!("[HTTP DEBUG] {} {}", method, url);
    if let Some(proxy) = proxy_settings {
        println!("[HTTP DEBUG] Proxy enabled: {} host: {} port: {} disable_cert_validation: {}", proxy.enabled, proxy.http_host, proxy.http_port, proxy.disable_cert_validation);
    } else {
        println!("[HTTP DEBUG] Proxy: None");
    }
    println!("[HTTP DEBUG] Request headers:");
    for (k, v) in headers.iter() {
        println!("  {}: {:?}", k, v);
    }
    if let Some(b) = body {
        println!("[HTTP DEBUG] Request body: {}", b);
    }
}

fn log_http_response(status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap, body: &str) {
    println!("[HTTP DEBUG] Response status: {}", status);
    println!("[HTTP DEBUG] Response headers:");
    for (k, v) in headers.iter() {
        println!("  {}: {:?}", k, v);
    }
    println!("[HTTP DEBUG] Response body (first 512 chars): {}", &body.chars().take(512).collect::<String>());
}

/// Starts EventQueueGet polling as a background task during login flow
async fn start_event_queue_polling(
    capabilities: &Capabilities,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
) -> Result<JoinHandle<()>, String> {
    let caps_clone = capabilities.clone();
    let proxy_clone = proxy_settings.cloned();
    
    let handle = tokio::spawn(async move {
        let _ = poll_event_queue(&caps_clone, udp_port, proxy_clone.as_ref(), |event| {
            info!("[EQG] Received event during login: {}", 
                  if event.len() > 200 { 
                      format!("{}...", &event[..200]) 
                  } else { 
                      event 
                  });
        }).await;
    });
    
    Ok(handle)
}

pub async fn login_to_secondlife(grid_uri: &str, req: &LoginRequest, proxy_settings: Option<&ProxySettings>, udp_port: u16) -> Result<LoginSessionInfo, String> {
    info!("[LOGIN] login_to_secondlife called. proxy_settings: {:?}", proxy_settings);
    // --- Build official viewer-matching fields ---
    let channel = "Second Life Release".to_string();
    let version = "7.1.15.15596336374".to_string();
    let platform = "mac".to_string();
    let platform_string = "macOS 12.7.4".to_string();
    let platform_version = "12.7.4".to_string();
    // Use dummy id0/mac for now (should be real hardware IDs)
    let id0 = "2cbb24b76a6a40fea1bff24b0ab32d08".to_string();
    let mac = "b91746d84dada8ffd383e1e9ce32649b".to_string();
    // Hash password as $1$md5(password)
    let md5 = format!("{:x}", md5::compute(&req.password));
    let passwd = format!("$1${}", md5);
    // Build XML body with these fields
    let options_xml: String = LoginRequest::default_options().iter()
        .map(|opt| format!("<value><string>{}</string></value>", opt))
        .collect::<Vec<_>>()
        .join("\n");
    let xml_body = format!(r#"<?xml version="1.0" ?>
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
          <member><name>options</name><value><array><data>{options_xml}</data></array></value></member>
          <member><name>passwd</name><value><string>{passwd}</string></value></member>
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
        channel = channel,
        extended_errors = req.extended_errors,
        first = req.first,
        host_id = req.host_id,
        id0 = id0,
        last = req.last,
        last_exec_duration = req.last_exec_duration,
        last_exec_event = req.last_exec_event,
        last_exec_session_id = req.last_exec_session_id,
        mac = mac,
        mfa_hash = req.mfa_hash,
        options_xml = options_xml,
        passwd = passwd,
        platform = platform,
        platform_string = platform_string,
        platform_version = platform_version,
        read_critical = req.read_critical,
        start = req.start,
        token = req.token,
        version = version,
    );
    eprintln!("[LOGIN XML BODY]\n{}", xml_body);
    let client = build_proxied_client(proxy_settings);
    println!("[DEBUG] Login POST will use proxy: {:?}", proxy_settings);
    println!("[DEBUG] Login POST URL: {}", grid_uri);
    // --- Actual login POST ---
    let mut req_builder = client
        .post(grid_uri)
        .header("Content-Type", "text/xml")
        // .header("User-Agent", "Second Life Release 7.1.15 (15596336374)")
        .header("User-Agent", "SecondLife/7.1.15.15596336374 (Second Life Release; default skin)")
        .header("Accept", "*/*")
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300");
    let request = req_builder.body(xml_body.clone()).build().map_err(|e| format!("Request build error: {e}"))?;
    println!("[DEBUG] Login POST headers:");
    for (k, v) in request.headers().iter() {
        println!("  {}: {:?}", k, v);
    }
    log_http_request("POST", grid_uri, proxy_settings, request.headers(), Some(&xml_body));
    let res = client.execute(request).await.map_err(|e| format!("HTTP error: {e}"))?;
    let status = res.status();
    let headers = res.headers().clone();
    let text = res.text().await.map_err(|e| format!("HTTP error: {e}"))?;
    log_http_response(status, &headers, &text);
    // eprintln!("[LOGIN RAW RESPONSE]\n{}", text);
    // Remove debug file output
    // eprintln!("[LOGIN RESPONSE] Raw body length: {}", text.len());
    // // Filter out large inventory-skel-lib and inventory-skeleton sections from debug print
    // let re = Regex::new(r"(?s)<member>\s*<name>(inventory-skel-lib|inventory-skeleton)</name>\s*<value>\s*<array>.*?</array>\s*</value>\s*</member>").unwrap();
    // let filtered_text = re.replace_all(&text, |caps: &regex::Captures| {
        // format!("<member><name>{}</name><value><array>[...omitted...]</array></value></member>", &caps[1])
    // });
    // Only print the filtered [LOGIN RESPONSE] log
    eprintln!("[LOGIN RESPONSE] HTTP status: {}", status);
    // eprintln!("[LOGIN RESPONSE] Raw body:\n{}", filtered_text);
    // eprintln!("[DEBUG] Full login response XML:\n{}", text);
    match parse_login_response(&text) {
        Ok(mut info) => {
            // Extract session cookie from Set-Cookie headers for my.secondlife.com requests
            for (name, value) in headers.iter() {
                if name == "set-cookie" {
                    if let Ok(cookie_str) = value.to_str() {
                        eprintln!("[DEBUG] Found Set-Cookie: {}", cookie_str);
                        if cookie_str.contains("agni_sl_session_id") {
                            info.session_cookie = Some(extract_cookie_kv(cookie_str));
                            eprintln!("[DEBUG] Extracted session cookie: {}", extract_cookie_kv(cookie_str));
                            break;
                        }
                    }
                }
            }
            // --- OpenID/capabilities step: MUST complete OpenID POST before UDP handshake ---
            if let Some(openid_token) = extract_openid_token(&text) {
                eprintln!("[DEBUG] Found openid_token: {}", openid_token);
                let openid_token = openid_token.replace("&amp;", "&");
                let openid_url = extract_openid_url(&text).unwrap_or_else(|| "https://id.secondlife.com/openid/webkit".to_string());
                let client = build_proxied_client(proxy_settings);
                let res = client
                    .post(&openid_url)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
                    .header("Connection", "keep-alive")
                    .header("Keep-alive", "300")
                    .body(openid_token.clone())
                    .send()
                    .await;
                match res {
                    Ok(resp) => {
                        eprintln!("[DEBUG] OpenID POST status: {}", resp.status());
                        let headers = resp.headers().clone();
                        for (k, v) in headers.iter() {
                            eprintln!("[DEBUG] OpenID POST header: {}: {:?}", k, v);
                        }
                        let _ = resp.text().await;
                    }
                    Err(e) => {
                        eprintln!("[DEBUG] OpenID POST error (non-blocking): {}", e);
                    }
                }
                
                // Fetch my.secondlife.com homepage (required for proper session establishment)
                if let Some(ref cookie) = info.session_cookie {
                    match fetch_my_secondlife_homepage(cookie, udp_port, proxy_settings).await {
                        Ok(response) => {
                            eprintln!("[DEBUG] my.secondlife.com GET successful (length: {} chars)", response.len());
                        }
                        Err(e) => {
                            eprintln!("[DEBUG] my.secondlife.com GET error (non-blocking): {}", e);
                        }
                    }
                } else {
                    eprintln!("[DEBUG] No session cookie available for my.secondlife.com GET");
                }
            }
            // After parsing login response and extracting seed_capability
            // Fetch seed capabilities if not present
            let mut capabilities = info.capabilities.clone();
            if capabilities.is_none() {
                match fetch_seed_capabilities(
                    &info.seed_capability,
                    udp_port,
                    proxy_settings,
                    info.session_cookie.as_deref(),
                ).await {
                    Ok(caps) => {
                        capabilities = Some(caps);
                    }
                    Err(e) => {
                        eprintln!("[CAPS] Failed to fetch seed capabilities: {}", e);
                    }
                }
            }
            
            // Start EventQueueGet polling if capabilities are available
            let _eq_task_handle = if let Some(ref caps) = capabilities {
                match start_event_queue_polling(caps, udp_port, proxy_settings).await {
                    Ok(handle) => {
                        info!("[LOGIN] ✅ EventQueueGet polling started during login flow");
                        Some(handle)
                    }
                    Err(e) => {
                        warn!("[LOGIN] ⚠️ Failed to start EventQueueGet polling: {}", e);
                        None
                    }
                }
            } else {
                warn!("[LOGIN] ⚠️ No capabilities available, skipping EventQueueGet polling");
                None
            };
            
            // --- Now start UDP handshake (after OpenID POST and EQG setup is done) ---
            let sim_addr = format!("{}:{}", info.sim_ip, info.sim_port).parse().unwrap();
            let session_id = uuid::Uuid::parse_str(&info.session_id).unwrap();
            let agent_id = uuid::Uuid::parse_str(&info.agent_id).unwrap();
            let circuit_code = info.circuit_code;
            // Use a default local port (0 = OS assigns)
            let udp_port = 0;
            let mut udp_transport = crate::networking::transport::UdpTransport::new(udp_port, sim_addr, proxy_settings).await.map_err(|e| format!("Failed to create UDP transport: {}", e))?;

            // Create agent_state for dynamic updates
            let agent_state = Arc::new(tokio::sync::Mutex::new(crate::networking::circuit::AgentState {
                position: (0.0, 0.0, 0.0),
                camera_at: (0.0, 0.0, 0.0),
                camera_eye: (0.0, 0.0, 0.0),
                controls: 0,
            }));
            // Create a Circuit for handshake management
            let mut circuit = crate::networking::circuit::Circuit::new_with_transport(
                Arc::new(tokio::sync::Mutex::new(udp_transport)),
                agent_state
            ).await.map_err(|e| e.to_string())?;
            
            // Handshake configuration is now handled by the Circuit's default implementation.
            // The default delay is 2000ms for compatibility, but can be overridden by
            // the SLV_HANDSHAKE_DELAY_MS environment variable if needed for performance testing.
            let position = (0.0, 0.0, 0.0);
            let look_at = (0.0, 0.0, 0.0);
            let throttle = [207360.0, 165376.0, 33075.19921875, 33075.19921875, 682700.75, 682700.75, 269312.0];
            let flags = 0; // Default flags
            let controls = 0; // Default controls
            let camera_at = (0.0, 0.0, 0.0);
            let camera_eye = (0.0, 0.0, 0.0);

            tokio::spawn(async move {
                // Start handshake sequence (only call once - state machine handles progression)
                circuit.advance_handshake(agent_id, session_id, circuit_code, position, look_at, throttle, flags, controls, camera_at, camera_eye, &sim_addr).await;
                // Start the receive loop to handle all incoming messages and handshake progression
                circuit.run_receive_loop(
                    agent_id,
                    session_id,
                    circuit_code,
                    position,
                    look_at,
                    throttle,
                    flags,
                    controls,
                    camera_at,
                    camera_eye,
                    &sim_addr
                ).await;
            });
            Ok(info)
        }
        Err(e) => Err(e),
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
        capabilities: None, // Initialize capabilities to None
        session_cookie: None, // Initialize session_cookie to None
    })
}

// Improved helper to extract openid_token from the login response XML using roxmltree robustly
fn extract_openid_token(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    for member in doc.descendants().filter(|n| n.has_tag_name("member")) {
        let mut name = None;
        let mut value = None;
        for child in member.children() {
            if child.has_tag_name("name") {
                name = child.text();
            }
            if child.has_tag_name("value") {
                // Look for <string> child or direct text
                for vchild in child.children() {
                    if vchild.has_tag_name("string") {
                        value = vchild.text();
                    }
                }
                if value.is_none() {
                    value = child.text();
                }
            }
        }
        if name == Some("openid_token") {
            return value.map(|s| s.to_string());
        }
    }
    None
}

// New: Extract openid_url from the login response XML
fn extract_openid_url(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    for member in doc.descendants().filter(|n| n.has_tag_name("member")) {
        let mut name = None;
        let mut value = None;
        for child in member.children() {
            if child.has_tag_name("name") {
                name = child.text();
            }
            if child.has_tag_name("value") {
                // Look for <string> child or direct text
                for vchild in child.children() {
                    if vchild.has_tag_name("string") {
                        value = vchild.text();
                    }
                }
                if value.is_none() {
                    value = child.text();
                }
            }
        }
        if name == Some("openid_url") {
            return value.map(|s| s.to_string());
        }
    }
    None
}

// Helper: Extract only the cookie name and value from a Set-Cookie header
fn extract_cookie_kv(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap_or("").trim().to_string()
}

// Parse LLSD XML capabilities response into Capabilities struct
fn parse_capabilities_response(xml: &str) -> Result<Capabilities, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let mut map = HashMap::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("map")) {
        let mut key = None;
        for child in node.children() {
            if child.has_tag_name("key") {
                key = child.text();
            }
            if child.has_tag_name("string") {
                if let Some(k) = key {
                    if let Some(v) = child.text() {
                        map.insert(k.to_string(), v.to_string());
                    }
                }
                key = None;
            }
        }
    }
    Ok(Capabilities { map })
}

/// Polls the EventQueueGet capability repeatedly and delivers events via a callback.
pub async fn poll_event_queue<F>(
    capabilities: &Capabilities,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
    mut on_event: F,
) -> Result<(), String>
where
    F: FnMut(String) + Send + 'static,
{
    let eq_url = match capabilities.map.get("EventQueueGet") {
        Some(url) => url,
        None => return Err("EventQueueGet capability not found".to_string()),
    };
    let client = build_proxied_client(proxy_settings);
    let mut ack: Option<i32> = Some(0); // Always send <ack>0> in first request for Hippolyzer compatibility
    loop {
        let payload = if let Some(ack_val) = ack {
            format!(r#"<?xml version="1.0" ?><llsd><map><key>ack</key><integer>{}</integer><key>done</key><boolean>false</boolean></map></llsd>"#, ack_val)
        } else {
            r#"<?xml version="1.0" ?><llsd><map><key>done</key><boolean>false</boolean></map></llsd>"#.to_string()
        };
        let resp = client
            .post(eq_url)
            .header("Accept", "application/llsd+xml")
            .header("Content-Type", "application/llsd+xml")
            .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
            .body(payload)
            .send()
            .await
            .map_err(|e| format!("EventQueueGet POST error: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("EventQueueGet read error: {e}"))?;
        if !status.is_success() {
            return Err(format!("EventQueueGet failed: HTTP {}", status));
        }
        // Parse <ack> from the response XML, if present
        let mut new_ack: Option<i32> = None;
        if let Ok(doc) = roxmltree::Document::parse(&text) {
            for node in doc.descendants() {
                if node.has_tag_name("key") && node.text() == Some("ack") {
                    if let Some(val_node) = node.next_sibling_element() {
                        if val_node.has_tag_name("integer") {
                            if let Some(val_text) = val_node.text() {
                                if let Ok(val) = val_text.parse::<i32>() {
                                    new_ack = Some(val);
                                }
                            }
                        }
                    }
                }
            }
        }
        ack = new_ack;
        on_event(text.clone());
        // Simulate long-polling delay
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

/// Posts to the seed_capability URL to fetch the capabilities map.
pub async fn fetch_seed_capabilities(
    seed_capability: &str,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
    openid_cookie: Option<&str>,
) -> Result<Capabilities, String> {
    let llsd_body = r#"<?xml version="1.0" ?>
<llsd>
  <array>
    <string>AbuseCategories</string>
    <string>AcceptFriendship</string>
    <string>AcceptGroupInvite</string>
    <string>AgentPreferences</string>
    <string>AgentProfile</string>
    <string>AgentState</string>
    <string>AttachmentResources</string>
    <string>AvatarPickerSearch</string>
    <string>AvatarRenderInfo</string>
    <string>CharacterProperties</string>
    <string>ChatSessionRequest</string>
    <string>CopyInventoryFromNotecard</string>
    <string>CreateInventoryCategory</string>
    <string>DeclineFriendship</string>
    <string>DeclineGroupInvite</string>
    <string>DispatchRegionInfo</string>
    <string>DirectDelivery</string>
    <string>EnvironmentSettings</string>
    <string>EstateAccess</string>
    <string>EstateChangeInfo</string>
    <string>EventQueueGet</string>
    <string>ExtEnvironment</string>
    <string>FetchLib2</string>
    <string>FetchLibDescendents2</string>
    <string>FetchInventory2</string>
    <string>FetchInventoryDescendents2</string>
    <string>IncrementCOFVersion</string>
    <string>RequestTaskInventory</string>
    <string>InventoryAPIv3</string>
    <string>LibraryAPIv3</string>
    <string>InterestList</string>
    <string>InventoryThumbnailUpload</string>
    <string>GetDisplayNames</string>
    <string>GetExperiences</string>
    <string>AgentExperiences</string>
    <string>FindExperienceByName</string>
    <string>GetExperienceInfo</string>
    <string>GetAdminExperiences</string>
    <string>GetCreatorExperiences</string>
    <string>ExperiencePreferences</string>
    <string>GroupExperiences</string>
    <string>UpdateExperience</string>
    <string>IsExperienceAdmin</string>
    <string>IsExperienceContributor</string>
    <string>RegionExperiences</string>
    <string>ExperienceQuery</string>
    <string>GetMetadata</string>
    <string>GetObjectCost</string>
    <string>GetObjectPhysicsData</string>
    <string>GroupAPIv1</string>
    <string>GroupMemberData</string>
    <string>GroupProposalBallot</string>
    <string>HomeLocation</string>
    <string>LandResources</string>
    <string>LSLSyntax</string>
    <string>MapLayer</string>
    <string>MapLayerGod</string>
    <string>MeshUploadFlag</string>
    <string>ModifyMaterialParams</string>
    <string>ModifyRegion</string>
    <string>NavMeshGenerationStatus</string>
    <string>NewFileAgentInventory</string>
    <string>ObjectAnimation</string>
    <string>ObjectMedia</string>
    <string>ObjectMediaNavigate</string>
    <string>ObjectNavMeshProperties</string>
    <string>ParcelPropertiesUpdate</string>
    <string>ParcelVoiceInfoRequest</string>
    <string>ProductInfoRequest</string>
    <string>ProvisionVoiceAccountRequest</string>
    <string>VoiceSignalingRequest</string>
    <string>ReadOfflineMsgs</string>
    <string>RegionObjects</string>
    <string>RegionSchedule</string>
    <string>RemoteParcelRequest</string>
    <string>RenderMaterials</string>
    <string>RequestTextureDownload</string>
    <string>ResourceCostSelected</string>
    <string>RetrieveNavMeshSrc</string>
    <string>SearchStatRequest</string>
    <string>SearchStatTracking</string>
    <string>SendPostcard</string>
    <string>SendUserReport</string>
    <string>SendUserReportWithScreenshot</string>
    <string>ServerReleaseNotes</string>
    <string>SetDisplayName</string>
    <string>SimConsoleAsync</string>
    <string>SimulatorFeatures</string>
    <string>StartGroupProposal</string>
    <string>TerrainNavMeshProperties</string>
    <string>TextureStats</string>
    <string>UntrustedSimulatorMessage</string>
    <string>UpdateAgentInformation</string>
    <string>UpdateAgentLanguage</string>
    <string>UpdateAvatarAppearance</string>
    <string>UpdateGestureAgentInventory</string>
    <string>UpdateGestureTaskInventory</string>
    <string>UpdateNotecardAgentInventory</string>
    <string>UpdateNotecardTaskInventory</string>
    <string>UpdateScriptAgent</string>
    <string>UpdateScriptTask</string>
    <string>UpdateSettingsAgentInventory</string>
    <string>UpdateSettingsTaskInventory</string>
    <string>UploadAgentProfileImage</string>
    <string>UpdateMaterialAgentInventory</string>
    <string>UpdateMaterialTaskInventory</string>
    <string>UploadBakedTexture</string>
    <string>UserInfo</string>
    <string>ViewerAsset</string>
    <string>ViewerBenefits</string>
    <string>ViewerMetrics</string>
    <string>ViewerStartAuction</string>
    <string>ViewerStats</string>
  </array>
</llsd>
"#;
    let client = build_proxied_client(proxy_settings);
    let url = Url::parse(seed_capability).ok();
    let host_header = url.as_ref().and_then(|u| {
        if let Some(port) = u.port() {
            Some(format!("{}:{}", u.host_str().unwrap_or(""), port))
        } else {
            Some(u.host_str().unwrap_or("").to_string())
        }
    });
    let mut req_builder = client
        .post(seed_capability)
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .header("User-Agent", "SecondLife/7.1.15.15596336374 (Second Life Release; default skin)")
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300");
    if let Some(ref host) = host_header {
        req_builder = req_builder.header("Host", host);
    }
    if let Some(cookie) = openid_cookie {
        req_builder = req_builder.header("Cookie", cookie);
    }
    let resp = req_builder.body(llsd_body).send().await.map_err(|e| format!("Seed capabilities POST error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Seed capabilities read error: {e}"))?;
    if !status.is_success() {
        return Err(format!("Seed capabilities POST failed: HTTP {}", status));
    }
    parse_capabilities_response(&text)
}

pub async fn fetch_tos_html(
    tos_id: &str,
    udp_port: Option<u16>,
    proxy_settings: Option<&ProxySettings>,
) -> Result<String, String> {
    let url = format!("https://secondlife.com/app/tos/tos.php?id={}", tos_id);
    let client = build_proxied_client(proxy_settings);
    let mut req = client.get(&url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml");
    if let Some(port) = udp_port {
        req = req.header("X-SecondLife-UDP-Listen-Port", port.to_string());
    }
    let request = req.build().map_err(|e| format!("Request build error: {e}"))?;
    log_http_request("GET", url.as_str(), proxy_settings, request.headers(), None);
    let resp = client.execute(request).await.map_err(|e| format!("TOS GET error: {e}"))?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let text = resp.text().await.map_err(|e| format!("TOS GET error: {e}"))?;
    log_http_response(status, &headers, &text);
    if !status.is_success() {
        return Err(format!("TOS GET failed: HTTP {}", status));
    }
    Ok(text)
}

/// Fetches SimulatorFeatures capability with correct UDP port header
pub async fn fetch_simulator_features(
    url: &str,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
) -> Result<String, String> {
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .get(url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .send()
        .await
        .map_err(|e| format!("SimulatorFeatures GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("SimulatorFeatures GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("SimulatorFeatures GET failed: HTTP {}", status));
    }
    Ok(text)
}

/// Fetches my.secondlife.com homepage with session cookie and UDP port
pub async fn fetch_my_secondlife_homepage(
    session_cookie: &str,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
) -> Result<String, String> {
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .get("https://my.secondlife.com/")
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "*/*")
        .header("Cookie", session_cookie)
        .header("User-Agent", "SecondLife/7.1.15.15596336374 (Second Life Release; default skin)")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .header("Host", "my.secondlife.com")
        .send()
        .await
        .map_err(|e| format!("my.secondlife.com GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("my.secondlife.com GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("my.secondlife.com GET failed: HTTP {}", status));
    }
    Ok(text)
}

/// Fetches NavMeshGenerationStatus capability with correct UDP port header
pub async fn fetch_navmesh_generation_status(
    url: &str,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
) -> Result<String, String> {
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .get(url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .send()
        .await
        .map_err(|e| format!("NavMeshGenerationStatus GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("NavMeshGenerationStatus GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("NavMeshGenerationStatus GET failed: HTTP {}", status));
    }
    Ok(text)
}

/// Posts AgentPreferences capability with correct UDP port header and LLSD XML body
pub async fn post_agent_preferences(
    url: &str,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
    llsd_body: &str,
) -> Result<String, String> {
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .post(url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .body(llsd_body.to_string())
        .send()
        .await
        .map_err(|e| format!("AgentPreferences POST error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("AgentPreferences POST error: {e}"))?;
    if !status.is_success() {
        return Err(format!("AgentPreferences POST failed: HTTP {}", status));
    }
    Ok(text)
}

/// Fetches ExtEnvironment capability with correct UDP port header
pub async fn fetch_ext_environment(
    url: &str,
    udp_port: u16,
    proxy_settings: Option<&ProxySettings>,
) -> Result<String, String> {
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .get(url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .send()
        .await
        .map_err(|e| format!("ExtEnvironment GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("ExtEnvironment GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("ExtEnvironment GET failed: HTTP {}", status));
    }
    Ok(text)
}

/// Fetches OpenID checklogin with correct UDP port, session cookie, and referer
pub async fn fetch_openid_checklogin(
    udp_port: u16,
    session_cookie: &str,
    proxy_settings: Option<&ProxySettings>,
    referer: &str,
) -> Result<String, String> {
    let url = "https://id.secondlife.com/openid/checklogin?return_to=https%3A%2F%2Fmy.secondlife.com%2F";
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .get(url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "*/*")
        .header("Cookie", session_cookie)
        .header("User-Agent", "SecondLife/7.1.15.15596336374 (Second Life Release; default skin)")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .header("Referer", referer)
        .send()
        .await
        .map_err(|e| format!("OpenID checklogin GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("OpenID checklogin GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("OpenID checklogin GET failed: HTTP {}", status));
    }
    Ok(text)
}

/// Fetches my.secondlife.com OpenID redirect with correct UDP port, session cookie, and referer
pub async fn fetch_my_secondlife_openid_redirect(
    udp_port: u16,
    session_cookie: &str,
    proxy_settings: Option<&ProxySettings>,
    referer: &str,
) -> Result<String, String> {
    let url = "https://my.secondlife.com/?openid_identifier=https%3A%2F%2Fid.secondlife.com%2Fid%2Ffreshbreath";
    let client = build_proxied_client(proxy_settings);
    let resp = client
        .get(url)
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .header("Accept", "*/*")
        .header("Cookie", session_cookie)
        .header("User-Agent", "SecondLife/7.1.15.15596336374 (Second Life Release; default skin)")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .header("Referer", referer)
        .send()
        .await
        .map_err(|e| format!("my.secondlife.com OpenID redirect GET error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("my.secondlife.com OpenID redirect GET error: {e}"))?;
    if !status.is_success() {
        return Err(format!("my.secondlife.com OpenID redirect GET failed: HTTP {}", status));
    }
    Ok(text)
}

#[cfg(test)]
mod proxy_tests {
    // ... removed example.com proxy test functions ...
}
