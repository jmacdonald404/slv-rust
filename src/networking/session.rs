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

pub async fn login_to_secondlife(grid_uri: &str, req: &LoginRequest) -> Result<LoginSessionInfo, String> {
    let client = Client::new();
    let res = client
        .post(grid_uri)
        .json(req)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;
    let text = res.text().await.map_err(|e| format!("HTTP error: {e}"))?;
    parse_login_response(&text)
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
                    match name.as_str() {
                        "agent_id" => agent_id = Some(value),
                        "session_id" => session_id = Some(value),
                        "secure_session_id" => secure_session_id = Some(value),
                        "sim_ip" => sim_ip = Some(value),
                        "sim_port" => sim_port = value.parse().ok(),
                        "circuit_code" => circuit_code = value.parse().ok(),
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
    if let (Some(agent_id), Some(session_id), Some(secure_session_id), Some(sim_ip), Some(sim_port), Some(circuit_code)) =
        (agent_id, session_id, secure_session_id, sim_ip, sim_port, circuit_code)
    {
        Ok(LoginSessionInfo {
            agent_id,
            session_id,
            secure_session_id,
            sim_ip,
            sim_port,
            circuit_code,
        })
    } else {
        Err("Missing required login fields in response".to_string())
    }
}
