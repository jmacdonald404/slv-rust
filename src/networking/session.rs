use reqwest::Client;
use serde::{Serialize, Deserialize};
use quick_xml::de::from_str;
use quick_xml::events::Event;
use quick_xml::Reader;
use crate::ui::proxy::ProxySettings;
use tracing::{info, warn};

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
    pub agent_id: String,
    pub session_id: String,
    pub secure_session_id: String,
    pub sim_ip: String,
    pub sim_port: u16,
    pub circuit_code: u32,
    // Add more fields as needed
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
    eprintln!("[LOGIN RESPONSE] HTTP status: {}", status);
    eprintln!("[LOGIN RESPONSE] Raw body:\n{}", text);
    match parse_login_response(&text) {
        Ok(info) => Ok(info),
        Err(e) => {
            eprintln!("[ERROR] Failed to parse login response: {}\nRaw body: {}", e, text);
            Err(format!("Failed to parse login response: {e}"))
        }
    }
}

fn parse_login_response(xml: &str) -> Result<LoginSessionInfo, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut agent_id = None;
    let mut session_id = None;
    let mut secure_session_id = None;
    let mut sim_ip = None;
    let mut sim_port = None;
    let mut circuit_code = None;
    let mut login_success = None;
    let mut error_message = None;
    let mut in_struct = false;
    let mut last_name = None;
    let mut last_value = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"struct" => {
                in_struct = true;
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"struct" => {
                break;
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"name" && in_struct => {
                last_name = Some(reader.read_text(e.name()).unwrap_or_default());
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"value" && in_struct => {
                last_value = Some(reader.read_text(e.name()).unwrap_or_default());
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"member" && in_struct => {
                if let (Some(name), Some(value)) = (last_name.take(), last_value.take()) {
                    match &*name {
                        "agent_id" => agent_id = Some(value),
                        "session_id" => session_id = Some(value),
                        "secure_session_id" => secure_session_id = Some(value),
                        "sim_ip" => sim_ip = Some(value),
                        "sim_port" => sim_port = value.parse().ok(),
                        "circuit_code" => circuit_code = value.parse().ok(),
                        "login" => login_success = Some(value.clone()),
                        "message" => error_message = Some(value.clone()),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }
    if let Some(login) = login_success {
        if login == "false" {
            // Login failed, return the error message if present
            if let Some(msg) = error_message {
                return Err(msg.to_string());
            } else {
                return Err("Login failed (no message)".to_string());
            }
        }
    }
    if let (Some(agent_id), Some(session_id), Some(secure_session_id), Some(sim_ip), Some(sim_port), Some(circuit_code)) =
        (agent_id, session_id, secure_session_id, sim_ip, sim_port, circuit_code)
    {
        Ok(LoginSessionInfo {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            secure_session_id: secure_session_id.to_string(),
            sim_ip: sim_ip.to_string(),
            sim_port,
            circuit_code,
        })
    } else {
        Err("Missing required login fields in response".to_string())
    }
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
