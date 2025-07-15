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
use std::collections::HashMap;
use md5;
use std::fs;
use reqwest::Certificate;

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

// Helper to always build a reqwest::Client with proxy settings if enabled
fn build_proxied_client(_proxy_settings: Option<&crate::ui::proxy::ProxySettings>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder();
    // Always use the proxy for this test
    builder = builder.proxy(reqwest::Proxy::all("http://127.0.0.1:9062").unwrap());
    let ca_cert = fs::read("src/assets/CA.pem").expect("Failed to read Hippolyzer CA cert");
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
    println!("[HTTP DEBUG] Client pointer (before test requests): {:p}", &client);
    // --- TEST HTTP/HTTPS REQUESTS ---
    // HTTP
    match client.get("http://example.com/").build() {
        Ok(request) => {
            log_http_request("GET", "http://example.com/", proxy_settings, request.headers(), None);
            match client.execute(request).await {
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let text = resp.text().await.unwrap_or_else(|_| "<body read error>".to_string());
                    log_http_response(status, &headers, &text);
                },
                Err(e) => println!("[HTTP DEBUG] HTTP test request error: {}", e),
            }
        },
        Err(e) => println!("[HTTP DEBUG] HTTP test request build error: {}", e),
    }
    // HTTPS
    match client.get("https://example.com/").build() {
        Ok(request) => {
            log_http_request("GET", "https://example.com/", proxy_settings, request.headers(), None);
            match client.execute(request).await {
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let text = resp.text().await.unwrap_or_else(|_| "<body read error>".to_string());
                    log_http_response(status, &headers, &text);
                },
                Err(e) => println!("[HTTP DEBUG] HTTPS test request error: {}", e),
            }
        },
        Err(e) => println!("[HTTP DEBUG] HTTPS test request build error: {}", e),
    }
    // --- END TEST HTTP/HTTPS REQUESTS ---
    println!("[HTTP DEBUG] Client pointer (before login POST): {:p}", &client);
    // --- Optionally, test POST to example.com instead of grid_uri ---
    // let test_post_url = "https://example.com/";
    // let mut req_builder = client
    //     .post(test_post_url)
    //     .header("Content-Type", "text/xml")
    //     .header("User-Agent", "SecondLife/6.6.14.581961 (Second Life Release; default)");
    // let request = req_builder.body(xml_body.clone()).build().map_err(|e| format!("Request build error: {e}"))?;
    // log_http_request("POST", test_post_url, proxy_settings, request.headers(), Some(&xml_body));
    // let res = client.execute(request).await.map_err(|e| format!("HTTP error: {e}"))?;
    // let status = res.status();
    // let headers = res.headers().clone();
    // let text = res.text().await.map_err(|e| format!("HTTP error: {e}"))?;
    // log_http_response(status, &headers, &text);
    // return Err("Test POST to example.com complete".to_string());
    // --- END test POST block ---
    // --- Actual login POST ---
    let mut req_builder = client
        .post(grid_uri)
        .header("Content-Type", "text/xml")
        .header("User-Agent", "Second Life Release 7.1.15 (15596336374)")
        .header("Accept", "*/*")
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300");
    let request = req_builder.body(xml_body.clone()).build().map_err(|e| format!("Request build error: {e}"))?;
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
        Ok(info) => {
            eprintln!("[DEBUG] Checking for openid_token in login response...");
            // Send OpenID POST with openid_token if present
            if let Some(openid_token) = extract_openid_token(&text) {
                eprintln!("[DEBUG] Found openid_token: {}", openid_token);
                let openid_token = openid_token.replace("&amp;", "&");
                let client = build_proxied_client(proxy_settings);
                // Second request: OpenID POST
                let res = client
                    .post("https://id.secondlife.com/openid/webkit")
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
                    .header("User-Agent", "SecondLife/6.6.14.581961 (Second Life Release; default)")
                    .body(openid_token.clone())
                    .send()
                    .await;
                let mut openid_cookie: Option<String> = None;
                match res {
                    Ok(resp) => {
                        eprintln!("[DEBUG] OpenID POST status: {}", resp.status());
                        if let Some(cookie) = resp.headers().get(reqwest::header::SET_COOKIE) {
                            eprintln!("[DEBUG] OpenID Set-Cookie: {:?}", cookie);
                            openid_cookie = Some(cookie.to_str().unwrap_or("").to_string());
                        }
                    }
                    Err(e) => {
                        eprintln!("[DEBUG] OpenID POST error: {}", e);
                    }
                }
                // Third request: Capabilities POST (as in progress.md)
                let client = build_proxied_client(proxy_settings);
                    let mut req_builder = client
                    .post(&info.seed_capability)
                        .header("Accept", "application/llsd+xml")
                        .header("Content-Type", "application/llsd+xml")
                        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
                        .header("User-Agent", "SecondLife/6.6.14.581961 (Second Life Release; default)");
                    // Forward OpenID cookie if present
                    if let Some(ref cookie) = openid_cookie {
                        req_builder = req_builder.header("Cookie", cookie);
                    }
                    let res3 = req_builder.send().await;
                    let mut capabilities: Option<Capabilities> = None;
                    match res3 {
                        Ok(resp) => {
                            eprintln!("[DEBUG] Capabilities POST status: {}", resp.status());
                            if let Ok(text) = resp.text().await {
                                if let Ok(caps) = parse_capabilities_response(&text) {
                                    eprintln!("[DEBUG] Parsed {} capabilities", caps.map.len());
                                    capabilities = Some(caps);
                                } else {
                                    eprintln!("[DEBUG] Could not parse capabilities response");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[DEBUG] Capabilities POST error: {}", e);
                        }
                    }
                    // Attach capabilities to session info
                    let mut info = info;
                    info.capabilities = capabilities;
                    // Store session cookie if present
                    if let Some(ref cookie) = openid_cookie {
                        info.session_cookie = Some(cookie.clone());
                }
                Ok(info)
            } else {
                Ok(info)
            }
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
    let mut ack: Option<i32> = None;
    loop {
        let payload = if let Some(ack_val) = ack {
            format!(r#"<?xml version=\"1.0\"?><llsd><map><key>ack</key><integer>{}</integer><key>done</key><boolean>false</boolean></map></llsd>"#, ack_val)
        } else {
            r#"<?xml version=\"1.0\"?><llsd><map><key>done</key><boolean>false</boolean></map></llsd>"#.to_string()
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
        // For now, just pass the raw XML to the callback
        on_event(text.clone());
        // TODO: Parse LLSD XML, extract ack if present, and handle events
        // For now, break if done (should loop forever in real client)
        // break;
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
    let llsd_body = r#"<?xml version=\"1.0\" ?>\n<llsd>\n<array>\n    <string>AbuseCategories</string>\n    <string>AcceptFriendship</string>\n    <string>AcceptGroupInvite</string>\n    <string>AgentPreferences</string>\n    <string>AgentProfile</string>\n    <string>AgentState</string>\n    <string>AttachmentResources</string>\n    <string>AvatarPickerSearch</string>\n    <string>AvatarRenderInfo</string>\n    <string>CharacterProperties</string>\n    <string>ChatSessionRequest</string>\n    <string>CopyInventoryFromNotecard</string>\n    <string>CreateInventoryCategory</string>\n    <string>DeclineFriendship</string>\n    <string>DeclineGroupInvite</string>\n    <string>DispatchRegionInfo</string>\n    <string>DirectDelivery</string>\n    <string>EnvironmentSettings</string>\n    <string>EstateAccess</string>\n    <string>EstateChangeInfo</string>\n    <string>EventQueueGet</string>\n    <string>ExtEnvironment</string>\n    <string>FetchLib2</string>\n    <string>FetchLibDescendents2</string>\n    <string>FetchInventory2</string>\n    <string>FetchInventoryDescendents2</string>\n    <string>IncrementCOFVersion</string>\n    <string>RequestTaskInventory</string>\n    <string>InventoryAPIv3</string>\n    <string>LibraryAPIv3</string>\n    <string>InterestList</string>\n    <string>InventoryThumbnailUpload</string>\n    <string>GetDisplayNames</string>\n    <string>GetExperiences</string>\n    <string>AgentExperiences</string>\n    <string>FindExperienceByName</string>\n    <string>GetExperienceInfo</string>\n    <string>GetAdminExperiences</string>\n    <string>GetCreatorExperiences</string>\n    <string>ExperiencePreferences</string>\n    <string>GroupExperiences</string>\n    <string>UpdateExperience</string>\n    <string>IsExperienceAdmin</string>\n    <string>IsExperienceContributor</string>\n    <string>RegionExperiences</string>\n    <string>ExperienceQuery</string>\n    <string>GetMetadata</string>\n    <string>GetObjectCost</string>\n    <string>GetObjectPhysicsData</string>\n    <string>GroupAPIv1</string>\n    <string>GroupMemberData</string>\n    <string>GroupProposalBallot</string>\n    <string>HomeLocation</string>\n    <string>LandResources</string>\n    <string>LSLSyntax</string>\n    <string>MapLayer</string>\n    <string>MapLayerGod</string>\n    <string>MeshUploadFlag</string>\n    <string>ModifyMaterialParams</string>\n    <string>ModifyRegion</string>\n    <string>NavMeshGenerationStatus</string>\n    <string>NewFileAgentInventory</string>\n    <string>ObjectAnimation</string>\n    <string>ObjectMedia</string>\n    <string>ObjectMediaNavigate</string>\n    <string>ObjectNavMeshProperties</string>\n    <string>ParcelPropertiesUpdate</string>\n    <string>ParcelVoiceInfoRequest</string>\n    <string>ProductInfoRequest</string>\n    <string>ProvisionVoiceAccountRequest</string>\n    <string>VoiceSignalingRequest</string>\n    <string>ReadOfflineMsgs</string>\n    <string>RegionObjects</string>\n    <string>RegionSchedule</string>\n    <string>RemoteParcelRequest</string>\n    <string>RenderMaterials</string>\n    <string>RequestTextureDownload</string>\n    <string>ResourceCostSelected</string>\n    <string>RetrieveNavMeshSrc</string>\n    <string>SearchStatRequest</string>\n    <string>SearchStatTracking</string>\n    <string>SendPostcard</string>\n    <string>SendUserReport</string>\n    <string>SendUserReportWithScreenshot</string>\n    <string>ServerReleaseNotes</string>\n    <string>SetDisplayName</string>\n    <string>SimConsoleAsync</string>\n    <string>SimulatorFeatures</string>\n    <string>StartGroupProposal</string>\n    <string>TerrainNavMeshProperties</string>\n    <string>TextureStats</string>\n    <string>UntrustedSimulatorMessage</string>\n    <string>UpdateAgentInformation</string>\n    <string>UpdateAgentLanguage</string>\n    <string>UpdateAvatarAppearance</string>\n    <string>UpdateGestureAgentInventory</string>\n    <string>UpdateGestureTaskInventory</string>\n    <string>UpdateNotecardAgentInventory</string>\n    <string>UpdateNotecardTaskInventory</string>\n    <string>UpdateScriptAgent</string>\n    <string>UpdateScriptTask</string>\n    <string>UpdateSettingsAgentInventory</string>\n    <string>UpdateSettingsTaskInventory</string>\n    <string>UploadAgentProfileImage</string>\n    <string>UpdateMaterialAgentInventory</string>\n    <string>UpdateMaterialTaskInventory</string>\n    <string>UploadBakedTexture</string>\n    <string>UserInfo</string>\n    <string>ViewerAsset</string>\n    <string>ViewerBenefits</string>\n    <string>ViewerMetrics</string>\n    <string>ViewerStartAuction</string>\n    <string>ViewerStats</string>\n  </array>\n</llsd>\n"#;
    let client = build_proxied_client(proxy_settings);
    let mut req_builder = client
        .post(seed_capability)
        .header("Accept", "application/llsd+xml")
        .header("Content-Type", "application/llsd+xml")
        .header("X-SecondLife-UDP-Listen-Port", udp_port.to_string())
        .header("User-Agent", "SecondLife/6.6.14.581961 (Second Life Release; default)");
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
    use super::*;
    #[tokio::test]
    async fn test_plain_http_proxy() {
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::http("http://127.0.0.1:9062").unwrap())
            .build()
            .unwrap();
        let resp = client.get("http://example.com/").send().await.unwrap();
        println!("[PROXY TEST] HTTP Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        println!("[PROXY TEST] HTTP Body (first 256 chars): {}", &body.chars().take(256).collect::<String>());
    }
    #[tokio::test]
    async fn test_https_proxy() {
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::http("http://127.0.0.1:9062").unwrap())
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let resp = client.get("https://example.com/").send().await.unwrap();
        println!("[PROXY TEST] HTTPS Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        println!("[PROXY TEST] HTTPS Body (first 256 chars): {}", &body.chars().take(256).collect::<String>());
    }
}
