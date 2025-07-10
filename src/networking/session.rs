use reqwest::Client;
use serde::{Serialize, Deserialize};
use quick_xml::de::from_str;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Serialize, Debug)]
pub struct LoginRequest {
    pub first: String,
    pub last: String,
    pub password: String,
    pub start: String,
    pub channel: String,
    pub version: String,
    pub platform: String,
    pub mac: String,
    pub id0: String,
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
    // This builds a minimal XML-RPC login request. Add more fields as needed.
    format!(r#"<?xml version="1.0" ?>
<methodCall>
  <methodName>login_to_simulator</methodName>
  <params>
    <param>
      <value>
        <struct>
          <member><name>first</name><value><string>{first}</string></value></member>
          <member><name>last</name><value><string>{last}</string></value></member>
          <member><name>passwd</name><value><string>{password}</string></value></member>
          <member><name>start</name><value><string>{start}</string></value></member>
          <member><name>channel</name><value><string>{channel}</string></value></member>
          <member><name>version</name><value><string>{version}</string></value></member>
          <member><name>platform</name><value><string>{platform}</string></value></member>
          <member><name>mac</name><value><string>{mac}</string></value></member>
          <member><name>id0</name><value><string>{id0}</string></value></member>
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#,
        first = req.first,
        last = req.last,
        password = req.password,
        start = req.start,
        channel = req.channel,
        version = req.version,
        platform = req.platform,
        mac = req.mac,
        id0 = req.id0,
    )
}

pub async fn login_to_secondlife(grid_uri: &str, req: &LoginRequest) -> Result<LoginSessionInfo, String> {
    let xml_body = build_login_xml(req);
    eprintln!("[LOGIN XML BODY]\n{}", xml_body);
    let client = Client::new();
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
