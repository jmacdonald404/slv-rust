// TODO: Set up egui main window integration
// TODO: Initialize egui context and handle UI events
// TODO: Implement HUD and settings panels here

use crate::ui::{UiState, LoginUiState, LoginProgress, LoginResult, UdpConnectionProgress};
use crate::networking::session::{login_to_secondlife, LoginRequest, LoginSessionInfo};
use crate::networking::circuit::Circuit;
use crate::networking::protocol::messages::Message;
use std::net::SocketAddr;
use crossbeam_channel::{unbounded, Sender, Receiver};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use crate::ui::chat;
use tokio::sync::Mutex;
use std::sync::Arc;
use eframe::egui;
use crate::networking::socks5_udp::Socks5UdpSocket;
use crate::networking::transport::{UdpTransport, UdpSocketExt};
use crate::config::settings;
use crate::networking::session::fetch_tos_html;
use scraper::{Html, Selector, ElementRef};
use egui::{RichText, Ui};

fn render_tos_html(ui: &mut Ui, html: &str) {
    let document = Html::parse_document(html);

    // Render headings
    for heading in document.select(&Selector::parse("h1, h2, h3, h4").unwrap()) {
        let text = heading.text().collect::<String>();
        ui.heading(text.trim());
    }

    // Render paragraphs (with bold/strong/italic detection)
    for para in document.select(&Selector::parse("p").unwrap()) {
        let mut text = String::new();
        let mut rich = RichText::new("");
        let mut is_bold = false;
        let mut is_italic = false;

        for node in para.children() {
            if let Some(elem) = node.value().as_element() {
                match elem.name() {
                    "strong" | "b" => {
                        is_bold = true;
                        text.push_str(&ElementRef::wrap(node).unwrap().text().collect::<String>());
                    }
                    "em" | "i" => {
                        is_italic = true;
                        text.push_str(&ElementRef::wrap(node).unwrap().text().collect::<String>());
                    }
                    "a" => {
                        // Render links as hyperlinks
                        let link = ElementRef::wrap(node).unwrap();
                        let href = link.value().attr("href").unwrap_or("#");
                        let link_text = link.text().collect::<String>();
                        ui.hyperlink_to(link_text.trim(), href);
                    }
                    _ => {
                        text.push_str(&ElementRef::wrap(node).unwrap().text().collect::<String>());
                    }
                }
            } else if let Some(txt) = node.value().as_text() {
                text.push_str(txt);
            }
        }

        if !text.trim().is_empty() {
            rich = RichText::new(text.trim());
            if is_bold {
                rich = rich.strong();
            }
            if is_italic {
                rich = rich.italics();
            }
            ui.label(rich);
        }
        ui.add_space(4.0);
    }

    // Render standalone links outside paragraphs
    for link in document.select(&Selector::parse("a").unwrap()) {
        let href = link.value().attr("href").unwrap_or("#");
        let link_text = link.text().collect::<String>();
        ui.hyperlink_to(link_text.trim(), href);
    }
}

pub struct UdpConnectResult {
    pub result: Result<std::sync::Arc<tokio::sync::Mutex<Circuit>>, String>,
}

pub fn show_main_window(ctx: &egui::Context, ui_state: &mut UiState) {
    // Poll for login result
    while let Ok(result) = ui_state.login_result_rx.try_recv() {
        match result.result {
            Ok(session_info) => {
                ui_state.login_progress = LoginProgress::Success;
                ui_state.login_ui_state = LoginUiState::MainApp;
                ui_state.login_state.session_info = Some(session_info.clone());
                // Start UDP connection
                if let (Ok(ip), port) = (session_info.sim_ip.parse(), session_info.sim_port) {
                    let sim_addr = SocketAddr::new(ip, port);
                    ui_state.udp_progress = UdpConnectionProgress::Connecting;
                    let udp_tx = ui_state.udp_connect_tx.clone();
                    let session_info = session_info.clone();
                    let proxy_settings = ui_state.proxy_settings.clone();
                    let handle = tokio::spawn(async move {
                        let socket_result: Result<Box<dyn UdpSocketExt>, String> = if proxy_settings.enabled {
                            match Socks5UdpSocket::connect(&proxy_settings.socks5_host, proxy_settings.socks5_port).await {
                                Ok(sock) => Ok(Box::new(sock)),
                                Err(e) => {
                                    tracing::error!("Failed to connect to SOCKS5 proxy: {}", e);
                                    Err(format!("Failed to connect to SOCKS5 proxy: {e}"))
                                }
                            }
                        } else {
                            match UdpTransport::new("0.0.0.0:0").await {
                                Ok(transport) => Ok(Box::new(transport)),
                                Err(e) => {
                                    tracing::error!("Failed to bind UDP socket: {}", e);
                                    Err(format!("Failed to bind UDP socket: {e}"))
                                }
                            }
                        };
                        let circuit_result = match socket_result {
                            Ok(socket) => Circuit::new_with_socket(socket).await.map_err(|e| format!("UDP error: {e}")),
                            Err(e) => Err(e),
                        };
                        match circuit_result {
                            Ok(mut circuit) => {
                                // Send UseCircuitCode handshake
                                let handshake = Message::UseCircuitCode {
                                    agent_id: session_info.agent_id.clone(),
                                    session_id: session_info.session_id.clone(),
                                    circuit_code: session_info.circuit_code,
                                };
                                // Send handshake to sim
                                let send_result = circuit.send_message(&handshake, &sim_addr).await;
                                if let Err(e) = send_result {
                                    let _ = udp_tx.send(UdpConnectResult { result: Err(format!("UDP handshake send error: {e}")) });
                                    return;
                                }
                                // Wait for handshake response (UseCircuitCodeReply)
                                let handshake_result = timeout(Duration::from_secs(5), circuit.recv_message()).await;
                                match handshake_result {
                                    Ok(Ok((_header, Message::UseCircuitCodeReply(success), _addr))) if success => {
                                        // Send CompleteAgentMovement after successful handshake
                                        let complete_agent_movement = Message::CompleteAgentMovement {
                                            agent_id: session_info.agent_id.clone(),
                                            session_id: session_info.session_id.clone(),
                                            circuit_code: session_info.circuit_code,
                                            position: (
                                                (session_info.region_x as f32) + 128.0,
                                                (session_info.region_y as f32) + 128.0,
                                                25.0,
                                            ),
                                            look_at: parse_look_at(&session_info.look_at),
                                        };
                                        let _ = circuit.send_message(&complete_agent_movement, &sim_addr).await;
                                        // Send AgentUpdate after CompleteAgentMovement
                                        let agent_update = Message::AgentUpdate {
                                            agent_id: session_info.agent_id.clone(),
                                            session_id: session_info.session_id.clone(),
                                            position: (
                                                (session_info.region_x as f32) + 128.0,
                                                (session_info.region_y as f32) + 128.0,
                                                25.0,
                                            ),
                                            camera_at: (0.0, 0.0, 1.0),
                                            camera_eye: (0.0, 0.0, 0.0),
                                            controls: 0,
                                        };
                                        let _ = circuit.send_message(&agent_update, &sim_addr).await;
                                        let _ = udp_tx.send(UdpConnectResult { result: Ok(std::sync::Arc::new(tokio::sync::Mutex::new(circuit))) });
                                    }
                                    Ok(Ok((_header, Message::UseCircuitCodeReply(success), _addr))) if !success => {
                                        let _ = udp_tx.send(UdpConnectResult { result: Err("Handshake rejected by simulator".to_string()) });
                                    }
                                    Ok(Ok((_header, msg, _addr))) => {
                                        let _ = udp_tx.send(UdpConnectResult { result: Err(format!("Unexpected handshake reply: {:?}", msg)) });
                                    }
                                    Ok(Err(e)) => {
                                        let _ = udp_tx.send(UdpConnectResult { result: Err(format!("UDP receive error: {e}")) });
                                    }
                                    Err(_) => {
                                        let _ = udp_tx.send(UdpConnectResult { result: Err("Handshake timed out".to_string()) });
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = udp_tx.send(UdpConnectResult { result: Err(e) });
                            }
                        }
                    });
                    ui_state.udp_connect_task = Some(handle);
                } else {
                    ui_state.udp_progress = UdpConnectionProgress::Error("Invalid sim IP/port".to_string());
                }
            }
            Err(err_msg) => {
                // Robust ToS/critical message detection
                if err_msg.starts_with("TOS_REQUIRED::") {
                    let message = err_msg.trim_start_matches("TOS_REQUIRED::").to_string();
                    ui_state.tos_required = true;
                    ui_state.tos_html = Some(message.clone()); // TODO: fetch real ToS HTML if available
                    ui_state.tos_message = Some(message);
                    // Block login until user accepts
                    ui_state.login_progress = LoginProgress::Idle;
                    ui_state.login_state.agree_to_tos_next_login = true;
                    ui_state.login_state.status_message = "You must accept the Terms of Service to continue.".to_string();
                    // TODO: Extract and store tos_id if available
                } else if err_msg.starts_with("CRITICAL_REQUIRED::") {
                    let message = err_msg.trim_start_matches("CRITICAL_REQUIRED::").to_string();
                    ui_state.tos_required = true;
                    ui_state.tos_html = Some(message.clone()); // TODO: fetch real critical message HTML if available
                    ui_state.tos_message = Some(message);
                    // Block login until user accepts
                    ui_state.login_progress = LoginProgress::Idle;
                    ui_state.login_state.read_critical_next_login = true; // Set read_critical for next login
                    ui_state.login_state.status_message = "You must read and accept a critical message to continue.".to_string();
                } else {
                    ui_state.login_progress = LoginProgress::Error(err_msg);
                }
            }
        }
    }

    // Poll for UDP connection result
    while let Ok(result) = ui_state.udp_connect_rx.try_recv() {
        match result.result {
            Ok(circuit_mutex) => {
                ui_state.udp_progress = UdpConnectionProgress::Connected;
                ui_state.udp_circuit = Some(circuit_mutex.clone());
                ui_state.login_ui_state = LoginUiState::LoadingWorld;
                // Spawn world entry listener task
                let world_entry_tx = ui_state.udp_connect_tx.clone();
                let circuit_mutex_clone = circuit_mutex.clone();
                let handle = tokio::spawn(async move {
                    // Wait for first message from sim (now: look for AgentMovementComplete)
                    let entry_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let mut circuit = circuit_mutex_clone.lock().await;
                        circuit.recv_message().await
                    }).await;
                    match entry_result {
                        Ok(Ok((_header, Message::AgentMovementComplete { agent_id: _, session_id: _ /* TODO: extract more fields */ }, _addr))) => {
                            // World entry success
                            let _ = world_entry_tx.send(UdpConnectResult { result: Ok(circuit_mutex_clone) });
                        }
                        Ok(Ok((_header, _msg, _addr))) => {
                            // Unexpected message, treat as error or ignore
                            let _ = world_entry_tx.send(UdpConnectResult { result: Err("Unexpected message instead of AgentMovementComplete".to_string()) });
                        }
                        Ok(Err(e)) => {
                            let _ = world_entry_tx.send(UdpConnectResult { result: Err(format!("UDP receive error: {e}")) });
                        }
                        Err(_) => {
                            let _ = world_entry_tx.send(UdpConnectResult { result: Err("World entry timed out".to_string()) });
                        }
                    }
                });
                ui_state.udp_connect_task = Some(handle);

                // Add to UiState:
                // pub chat_event_rx: Option<Receiver<(String, String)>>,
                // pub chat_event_tx: Option<Sender<(String, String)>>,

                // In the UDP connection result handler, after world entry:
                // let (chat_event_tx, chat_event_rx) = unbounded();
                // ui_state.chat_event_tx = Some(chat_event_tx.clone());
                // ui_state.chat_event_rx = Some(chat_event_rx);
                // let mut circuit = circuit.clone();
                // tokio::spawn(async move {
                //     loop {
                //         if let Ok((_header, msg, _addr)) = circuit.recv_message().await {
                //             if let Message::ChatFromSimulator { sender, message, .. } = msg {
                //                 let _ = chat_event_tx.send((sender, message));
                //             }
                //         }
                //     }
                // });
            }
            Err(msg) => {
                ui_state.udp_progress = UdpConnectionProgress::Error(msg);
            }
        }
    }

    // Poll for UI events from async tasks
    while let Ok(event) = ui_state.ui_event_rx.try_recv() {
        match event {
            crate::ui::UiEvent::ShowTos { tos_id, tos_html, message } => {
                ui_state.tos_required = true;
                ui_state.tos_id = Some(tos_id);
                ui_state.tos_html = Some(tos_html);
                ui_state.tos_message = Some(message);
            }
            // Handle other events as needed
        }
    }

    // Preferences modal stub
    let mut prefs_open = ui_state.login_state.prefs_modal_open;
    let mut should_close = false;
    if prefs_open {
        egui::Window::new("Preferences")
            .collapsible(false)
            .resizable(false)
            .open(&mut prefs_open)
            .show(ctx, |ui| {
                let mut changed = false;
                ui.heading("Proxy Settings");
                ui.separator();
                changed |= ui.checkbox(&mut ui_state.proxy_settings.enabled, "Enable Proxy").changed();
                if ui_state.proxy_settings.enabled {
                    ui.horizontal(|ui| {
                        ui.label("SOCKS5 Host:");
                        changed |= ui.text_edit_singleline(&mut ui_state.proxy_settings.socks5_host).changed();
                        ui.label("Port:");
                        changed |= ui.add(egui::DragValue::new(&mut ui_state.proxy_settings.socks5_port).clamp_range(1..=65535)).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("HTTP Proxy Host:");
                        changed |= ui.text_edit_singleline(&mut ui_state.proxy_settings.http_host).changed();
                        ui.label("Port:");
                        changed |= ui.add(egui::DragValue::new(&mut ui_state.proxy_settings.http_port).clamp_range(1..=65535)).changed();
                    });
                    changed |= ui.checkbox(&mut ui_state.proxy_settings.disable_cert_validation, "Disable HTTPS Certificate Validation").changed();
                }
                if changed {
                    let _ = settings::save_general_settings(&ui_state.preferences, &ui_state.proxy_settings);
                }
                ui.separator();
                if ui.button("Close").clicked() {
                    should_close = true;
                }
                // A. Add a Test ToS Modal button for manual testing
                if ui.button("Test ToS Modal").clicked() {
                    let ui_event_tx = ui_state.ui_event_tx.clone();
                    tokio::spawn(async move {
                        let tos_id = "5f4c3d82d7f18c19a1a2d23331c9ac36";
                        match fetch_tos_html(tos_id, None, None).await {
                            Ok(tos_html) => {
                                let _ = ui_event_tx.send(crate::ui::UiEvent::ShowTos {
                                    tos_id: tos_id.to_string(),
                                    tos_html,
                                    message: "Test ToS".to_string(),
                                });
                            }
                            Err(e) => {
                                eprintln!("[TOS TEST] Failed to fetch ToS: {}", e);
                            }
                        }
                    });
                }
            });
        if should_close {
            prefs_open = false;
        }
        ui_state.login_state.prefs_modal_open = prefs_open;
    }

    // ToS modal
    if ui_state.tos_required {
        egui::Window::new("Terms of Service")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([600.0, 500.0])
            .show(ctx, |ui| {
                if let Some(html) = &ui_state.tos_html {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        render_tos_html(ui, html);
                    });
                } else {
                    ui.label("Loading ToS...");
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.button("I Agree").clicked() {
                        ui_state.tos_required = false;
                        ui_state.tos_html = None;
                        ui_state.tos_id = None;
                        ui_state.tos_message = None;
                        ui_state.login_state.agree_to_tos_next_login = true;
                    }
                    if ui.button("Decline").clicked() {
                        ui_state.tos_required = false;
                        ui_state.tos_html = None;
                        ui_state.tos_id = None;
                        ui_state.tos_message = None;
                    }
                });
            });
    }

    // In show_main_window, poll chat_event_rx and append to chat_messages:
    // if let Some(chat_event_rx) = &ui_state.chat_event_rx {
    //     while let Ok((sender, message)) = chat_event_rx.try_recv() {
    //         chat::append_incoming_chat(&mut ui_state.chat_messages, &sender, &message);
    //     }
    // }

    match ui_state.login_ui_state {
        LoginUiState::LoginSplash => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slv-rust Login");
                    ui.add_space(16.0);

                    // Username field
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut ui_state.login_state.username);
                    ui.add_space(8.0);

                    // Password field
                    ui.label("Password:");
                    ui.add(egui::TextEdit::singleline(&mut ui_state.login_state.password).password(true));
                    ui.add_space(16.0);

                    // Login button (disabled until fields are filled)
                    let login_enabled = !ui_state.login_state.username.is_empty() && !ui_state.login_state.password.is_empty() && matches!(ui_state.login_progress, LoginProgress::Idle);
                    if ui.add_enabled(login_enabled, egui::Button::new("Login")).clicked() || ui_state.login_state.agree_to_tos_next_login {
                        // Parse username into first/last ("First Last" or "first.last" or just "First")
                        let (first, last) = if ui_state.login_state.username.contains('.') {
                            let mut parts = ui_state.login_state.username.splitn(2, '.');
                            (
                                parts.next().unwrap_or("").to_string(),
                                parts.next().unwrap_or("Resident").to_string(),
                            )
                        } else {
                            let mut parts = ui_state.login_state.username.split_whitespace();
                            let first = parts.next().unwrap_or("").to_string();
                            let last = parts.next().unwrap_or("Resident").to_string();
                            (first, last)
                        };
                        let password = ui_state.login_state.password.clone();
                        let agree_to_tos = if ui_state.login_state.agree_to_tos_next_login { 1 } else { 0 };
                        let req = LoginRequest {
                            first,
                            last,
                            password,
                            start: "last".to_string(),
                            channel: "slv-rust".to_string(),
                            version: "0.3.0-alpha".to_string(),
                            platform: "linux".to_string(), // TODO: detect platform
                            platform_string: "macOS 12.7.4".to_string(), // TODO: real platform string
                            platform_version: "12.7.4".to_string(), // TODO: real version
                            mac: "00:00:00:00:00:00".to_string(), // TODO: real MAC
                            id0: "00000000-0000-0000-0000-000000000000".to_string(), // TODO: real id0
                            agree_to_tos,
                            address_size: 64,
                            extended_errors: 1,
                            host_id: String::new(),
                            last_exec_duration: 30,
                            last_exec_event: 0,
                            last_exec_session_id: "00000000-0000-0000-0000-000000000000".to_string(),
                            mfa_hash: String::new(),
                            token: String::new(),
                            read_critical: if ui_state.login_state.read_critical_next_login { 1 } else { 0 },
                            options: LoginRequest::default_options(),
                        };
                        let grid_uri = "https://login.agni.lindenlab.com/cgi-bin/login.cgi".to_string();
                        ui_state.login_progress = LoginProgress::InProgress;
                        let tx = ui_state.login_result_tx.clone();
                        let proxy_settings = ui_state.proxy_settings.clone();
                        let ui_event_tx = ui_state.ui_event_tx.clone();
                        ui_state.login_state.agree_to_tos_next_login = false;
                        // Spawn async login task
                        let handle = tokio::spawn(async move {
                            eprintln!("[LOGIN TASK] Starting login for: first='{}', last='{}'", req.first, req.last);
                            let result = login_to_secondlife(&grid_uri, &req, Some(&proxy_settings)).await;
                            match &result {
                                Ok(session_info) => {
                                    eprintln!("[LOGIN SUCCESS] agent_id={}, session_id={}", session_info.agent_id, session_info.session_id);
                                }
                                Err(err_msg) => {
                                    // Check for TOS_REQUIRED error
                                    if let Some(rest) = err_msg.strip_prefix("TOS_REQUIRED:") {
                                        let mut parts = rest.splitn(2, ':');
                                        let tos_id = parts.next().unwrap_or("").to_string();
                                        let message = parts.next().unwrap_or("").to_string();
                                        // B. If no tos_id, use a random/test one
                                        let tos_id = if tos_id.is_empty() { "5f4c3d82d7f18c19a1a2d23331c9ac36".to_string() } else { tos_id };
                                        // C. Log fetch errors
                                        match fetch_tos_html(&tos_id, None, Some(&proxy_settings)).await {
                                            Ok(tos_html) => {
                                                let _ = ui_event_tx.send(crate::ui::UiEvent::ShowTos {
                                                    tos_id,
                                                    tos_html,
                                                    message,
                                                });
                                            }
                                            Err(e) => {
                                                eprintln!("[TOS] Failed to fetch ToS: {}", e);
                                            }
                                        }
                                    } else {
                                        eprintln!("[LOGIN ERROR] {}", err_msg);
                                    }
                                }
                            }
                            let login_result = LoginResult { result };
                            let _ = tx.send(login_result);
                        });
                        ui_state.login_task = Some(handle);
                    }
                    ui.add_space(8.0);

                    // Preferences button
                    if ui.button("Preferences").clicked() {
                        ui_state.login_state.prefs_modal_open = true;
                    }
                    ui.add_space(16.0);

                    // Status area for login progress/errors
                    match &ui_state.login_progress {
                        LoginProgress::Idle => ui.label("Status: Ready"),
                        LoginProgress::InProgress => ui.label("Status: Logging in..."),
                        LoginProgress::Success => ui.label("Status: Login successful!"),
                        LoginProgress::Error(msg) => ui.colored_label(egui::Color32::RED, format!("Status: Error: {}", msg)),
                    };
                });
            });
        }
        LoginUiState::MainApp => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Main App UI (stub)");
                // Show UDP connection status
                match &ui_state.udp_progress {
                    UdpConnectionProgress::NotStarted => ui.label("UDP: Not started"),
                    UdpConnectionProgress::Connecting => ui.label("UDP: Connecting..."),
                    UdpConnectionProgress::Connected => ui.label("UDP: Connected!"),
                    UdpConnectionProgress::Error(msg) => ui.label(format!("UDP: Error: {}", msg)),
                };
            });
        }
        LoginUiState::LoadingWorld => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Loading world...");
                ui.label("Waiting for region/agent data from simulator...");
            });
        }
        LoginUiState::InWorld => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("In World (stub)");
                ui.label("You are now in the virtual world!");
                ui.separator();
                ui.label("[Chat panel placeholder]");
                ui.label("[Inventory panel placeholder]");
                ui.label("[Preferences panel placeholder]");
                ui.separator();
                if ui.button("Logout").clicked() {
                    ui_state.login_state.status_message = "User requested logout.".to_string();
                    ui_state.login_ui_state = crate::ui::LoginUiState::LoginSplash;
                    ui_state.logout_requested = true;
                }
            });
        }
    }
}

// Spawns a UDP connection task and returns a handle (stub for now)
pub fn udp_connect_task(sim_addr: SocketAddr, session_info: &LoginSessionInfo, _ctx: egui::Context) {
    // TODO: Actually spawn a tokio task, create Circuit, perform handshake, and update UI state via channel/interior mutability
    println!("Would connect UDP to {} with session info: {:?}", sim_addr, session_info);
}

// Helper function to parse look_at string
fn parse_look_at(s: &str) -> (f32, f32, f32) {
    let s = s.trim_matches(['[', ']'].as_ref());
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 3 {
        let x = parts[0].trim_start_matches('r').parse().unwrap_or(0.0);
        let y = parts[1].trim_start_matches('r').parse().unwrap_or(0.0);
        let z = parts[2].trim_start_matches('r').parse().unwrap_or(0.0);
        (x, y, z)
    } else {
        (0.0, 1.0, 0.0)
    }
}
