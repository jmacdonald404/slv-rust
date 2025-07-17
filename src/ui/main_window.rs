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
use crate::networking::transport::UdpTransport;
use crate::config::settings;
use crate::networking::session::fetch_tos_html;
use scraper::{Html, Selector, ElementRef};
use egui::{RichText, Ui};
use bytes::BufMut;
use crate::ui::AgentState;
use roxmltree::Document;
use rand::Rng;
use std::net::UdpSocket as StdUdpSocket;
use crate::utils::lludp::{LluPacket, LluPacketFlags};
use tokio::sync::oneshot;

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

fn parse_agent_state_update(llsd_xml: &str) -> Option<AgentState> {
    let doc = Document::parse(llsd_xml).ok()?;
    let mut state = AgentState::default();
    let map = doc.descendants().find(|n| n.has_tag_name("map"))?;
    for (k, v) in map.children().collect::<Vec<_>>().chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].text(), &c[1])) {
        match k? {
            "can_modify_navmesh" => state.can_modify_navmesh = v.text().unwrap_or("") == "true",
            "has_modified_navmesh" => state.has_modified_navmesh = v.text().unwrap_or("") == "true",
            "preferences" => {
                for (pk, pv) in v.children().collect::<Vec<_>>().chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].text(), &c[1])) {
                    match pk? {
                        "god_level" => state.god_level = pv.text().unwrap_or("0").parse().unwrap_or(0),
                        "hover_height" => state.hover_height = pv.text().unwrap_or("0.0").parse().unwrap_or(0.0),
                        "language" => state.language = pv.text().unwrap_or("").to_string(),
                        "language_is_public" => state.language_is_public = pv.text().unwrap_or("") == "true",
                        "access_prefs" => {
                            for (ak, av) in pv.children().collect::<Vec<_>>().chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].text(), &c[1])) {
                                if ak? == "max" {
                                    state.access_prefs_max = av.text().unwrap_or("").to_string();
                                }
                            }
                        }
                        "default_object_perm_masks" => {
                            let mut everyone = 0;
                            let mut group = 0;
                            let mut next_owner = 0;
                            for (dk, dv) in pv.children().collect::<Vec<_>>().chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].text(), &c[1])) {
                                match dk? {
                                    "Everyone" => everyone = dv.text().unwrap_or("0").parse().unwrap_or(0),
                                    "Group" => group = dv.text().unwrap_or("0").parse().unwrap_or(0),
                                    "NextOwner" => next_owner = dv.text().unwrap_or("0").parse().unwrap_or(0),
                                    _ => {}
                                }
                            }
                            state.default_object_perm_masks = (everyone, group, next_owner);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Some(state)
}

fn handle_agent_state_update_event(llsd_xml: &str, ui_state: &mut UiState) {
    if let Some(agent_state) = parse_agent_state_update(llsd_xml) {
        ui_state.agent_state = Some(agent_state.clone());
        println!("[AgentStateUpdate] Parsed: {:?}", agent_state);
    }
}

pub struct UdpConnectResult {
    pub result: Result<std::sync::Arc<tokio::sync::Mutex<Circuit>>, String>,
}

// Remove pick_random_udp_port from this file except for UiState::default.
// Everywhere a UDP port is needed (UDP socket, HTTP requests), use ui_state.session_udp_port.

pub fn show_main_window(ctx: &egui::Context, ui_state: &mut UiState) {
    // Poll for login result
    while let Ok(result) = ui_state.login_result_rx.try_recv() {
        match result.result {
            Ok(session_info) => {
                ui_state.login_progress = LoginProgress::Success;
                ui_state.login_ui_state = LoginUiState::MainApp;
                ui_state.login_state.session_info = Some(session_info.clone());
                // --- Wait for login HTTP and OpenID POST to complete before UDP/EQ ---
                println!("[DEBUG] Login HTTP and OpenID POST complete. Preparing to start UDP handshake and EQ polling...");
                // Add a small delay to ensure proxy can process login
                std::thread::sleep(std::time::Duration::from_millis(100));
                println!("[DEBUG] Starting UDP handshake and EQ polling now.");
                // Start UDP connection
                if let (Ok(ip), port) = (session_info.sim_ip.parse(), session_info.sim_port) {
                    let sim_addr = SocketAddr::new(ip, port);
                    ui_state.udp_progress = UdpConnectionProgress::Connecting;
                    let udp_connect_tx = ui_state.udp_connect_tx.clone();
                    let session_udp_port = ui_state.session_udp_port;
                    let proxy_settings = ui_state.proxy_settings.clone();
                    let session_info = session_info.clone();
                    let ui_event_tx = ui_state.ui_event_tx.clone();
                    // --- Coordination channels ---
                    let (udp_handshake_tx, mut udp_handshake_rx) = oneshot::channel();
                    let (eq_ready_tx, mut eq_ready_rx) = oneshot::channel();
                    // --- Start EQ polling ---
                    let eq_caps_info = session_info.clone();
                    let eq_proxy_settings = proxy_settings.clone();
                    let eq_ui_event_tx = ui_event_tx.clone();
                    tokio::spawn(async move {
                        let mut capabilities = eq_caps_info.capabilities.clone();
                        if capabilities.is_none() {
                            match crate::networking::session::fetch_seed_capabilities(
                                &eq_caps_info.seed_capability,
                                session_udp_port,
                                Some(&eq_proxy_settings),
                                eq_caps_info.session_cookie.as_deref(),
                            ).await {
                                Ok(caps) => {
                                    capabilities = Some(caps);
                                }
                                Err(e) => {
                                    eprintln!("[CAPS] Failed to fetch seed capabilities: {}", e);
                                }
                            }
                        }
                        if let Some(caps) = capabilities {
                            let mut eq_ready_tx = Some(eq_ready_tx);
                            let eq_ui_event_tx = eq_ui_event_tx.clone();
                            let _ = crate::networking::session::poll_event_queue(&caps, session_udp_port, Some(&eq_proxy_settings), move |event_xml| {
                                let _ = eq_ui_event_tx.send(crate::ui::UiEvent::AgentStateUpdate(event_xml.clone()));
                                if (event_xml.contains("EnableSimulator") || event_xml.contains("RegionHandshake")) {
                                    if let Some(tx) = eq_ready_tx.take() {
                                        let _ = tx.send(());
                                    }
                                }
                            }).await;
                        }
                    });
                    // --- Start UDP handshake ---
                    let udp_connect_tx2 = udp_connect_tx.clone();
                    let udp_handshake_tx2: tokio::sync::oneshot::Sender<()> = udp_handshake_tx;
                    tokio::spawn(async move {
                        let sim_addr = SocketAddr::new(session_info.sim_ip.parse().unwrap(), session_info.sim_port);
                        let local_udp_port = session_udp_port;
                        let udp_tx = udp_connect_tx2;
                        let proxy_settings = proxy_settings;
                        let session_info = session_info;
                            match crate::networking::transport::UdpTransport::new(local_udp_port, sim_addr, Some(&proxy_settings)).await {
                            Ok(mut udp) => {
                                    // --- Begin handshake: strictly follow message_template.msg protocol ---
                                    let session_id = uuid::Uuid::parse_str(&session_info.session_id).unwrap_or_default();
                                    let agent_id = uuid::Uuid::parse_str(&session_info.agent_id).unwrap_or_default();
                                    let circuit_code = session_info.circuit_code;
                                // Handshake is now managed by Circuit state machine
                                }
                                Err(e) => {
                                    let _ = udp_tx.send(UdpConnectResult { result: Err(format!("Failed to bind UDP socket: {e}")) });
                            }
                        }
                    });
                    // --- Wait for both handshake and EQ ready, then set InWorld ---
                    let ui_event_tx2 = ui_state.ui_event_tx.clone();
                    tokio::spawn(async move {
                        let _ = udp_handshake_rx.await;
                        let _ = eq_ready_rx.await;
                        let _ = ui_event_tx2.send(crate::ui::UiEvent::InWorldReady);
                    });
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
                let session_info = ui_state.login_state.session_info.clone();
                let proxy_settings = ui_state.proxy_settings.clone();
                let ui_event_tx = ui_state.ui_event_tx.clone();
                let session_udp_port = ui_state.session_udp_port;
                let handle = tokio::spawn(async move {
                    // Wait for first message from sim (now: look for AgentMovementComplete)
                    let entry_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        let mut circuit = circuit_mutex_clone.lock().await;
                        circuit.recv_message().await
                    }).await;
                    match entry_result {
                        Ok(Ok((_header, Message::AgentMovementComplete { agent_id: _, session_id: _ /* TODO: extract more fields */ }, _addr))) => {
                            // World entry success
                            let _ = world_entry_tx.send(UdpConnectResult { result: Ok(circuit_mutex_clone.clone()) });
                            // --- Region Handshake sequence ---
                            let mut circuit = circuit_mutex_clone.lock().await;
                            // Wait for RegionHandshake
                            if let Ok((_header, msg, addr)) = circuit.recv_message().await {
                                if let Message::RegionHandshake { .. } = msg {
                                    // Send RegionHandshakeReply
                                    let reply = Message::RegionHandshakeReply {
                                        agent_id: session_info.as_ref().map(|s| s.agent_id.clone()).unwrap_or_default(),
                                        session_id: session_info.as_ref().map(|s| s.session_id.clone()).unwrap_or_default(),
                                        flags: 0x07, // SUPPORTS_SELF_APPEARANCE | VOCACHE_CULLING_ENABLED (example)
                                    };
                                    let _ = circuit.send_message(&reply, &addr).await;
                                    // Send AgentThrottle
                                    let throttle = Message::AgentThrottle {
                                        agent_id: session_info.as_ref().map(|s| s.agent_id.clone()).unwrap_or_default(),
                                        session_id: session_info.as_ref().map(|s| s.session_id.clone()).unwrap_or_default(),
                                        circuit_code: session_info.as_ref().map(|s| s.circuit_code).unwrap_or(0),
                                        throttle: [1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0], // placeholder values
                                    };
                                    let _ = circuit.send_message(&throttle, &addr).await;
                                    // Send AgentUpdate
                                    let update = Message::AgentUpdate {
                                        agent_id: session_info.as_ref().map(|s| s.agent_id.clone()).unwrap_or_default(),
                                        session_id: session_info.as_ref().map(|s| s.session_id.clone()).unwrap_or_default(),
                                        position: (128.0, 128.0, 25.0), // placeholder
                                        camera_at: (0.0, 1.0, 0.0),
                                        camera_eye: (128.0, 128.0, 25.0),
                                        controls: 0,
                                    };
                                    let _ = circuit.send_message(&update, &addr).await;
                                }
                            }
                            // --- Fetch seed capabilities and start event queue polling ---
                            if let Some(session_info) = &session_info {
                                let mut capabilities = session_info.capabilities.clone();
                                // If not present, fetch them
                                if capabilities.is_none() {
                                    match crate::networking::session::fetch_seed_capabilities(
                                        &session_info.seed_capability,
                                        session_udp_port,
                                        Some(&proxy_settings),
                                        session_info.session_cookie.as_deref(),
                                    ).await {
                                        Ok(caps) => {
                                            capabilities = Some(caps);
                                        }
                                        Err(e) => {
                                            eprintln!("[CAPS] Failed to fetch seed capabilities: {}", e);
                                        }
                                    }
                                }
                                if let Some(caps) = capabilities {
                                    // Start event queue polling
                                    let ui_event_tx = ui_event_tx.clone();
                                    tokio::spawn(async move {
                                        let _ = crate::networking::session::poll_event_queue(&caps, session_udp_port, Some(&proxy_settings), move |event_xml| {
                                            // For now, just forward the raw XML as an AgentStateUpdate event
                                            let _ = ui_event_tx.send(crate::ui::UiEvent::AgentStateUpdate(event_xml));
                                        }).await;
                                    });
                                }
                            }
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
            crate::ui::UiEvent::AgentStateUpdate(llsd_xml) => {
                handle_agent_state_update_event(&llsd_xml, ui_state);
            }
            crate::ui::UiEvent::InWorldReady => {
                ui_state.login_ui_state = LoginUiState::InWorld;
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
                // Show full preferences panel (including UDP test)
                crate::ui::preferences::show_preferences_panel(ctx, &mut ui_state.preferences, false);
                ui.separator();
                ui.heading("Proxy Settings");
                let mut changed = false;
                changed |= ui.checkbox(&mut ui_state.proxy_settings.enabled, "Enable Proxy").changed();
                if ui_state.proxy_settings.enabled {
                    ui.horizontal(|ui| {
                        ui.label("SOCKS5 Host:");
                        changed |= ui.text_edit_singleline(&mut ui_state.proxy_settings.socks5_host).changed();
                        ui.label("Port:");
                        changed |= ui.add(egui::DragValue::new(&mut ui_state.proxy_settings.socks5_port).range(1..=65535)).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("HTTP Proxy Host:");
                        changed |= ui.text_edit_singleline(&mut ui_state.proxy_settings.http_host).changed();
                        ui.label("Port:");
                        changed |= ui.add(egui::DragValue::new(&mut ui_state.proxy_settings.http_port).range(1..=65535)).changed();
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
                        // Revert login endpoint to HTTPS for official test
                        let grid_uri = "https://login.agni.lindenlab.com/cgi-bin/login.cgi".to_string();
                        ui_state.login_progress = LoginProgress::InProgress;
                        let session_udp_port = ui_state.session_udp_port;
                        let tx = ui_state.login_result_tx.clone();
                        let proxy_settings = ui_state.proxy_settings.clone();
                        let ui_event_tx = ui_state.ui_event_tx.clone();
                        let agree_to_tos_next_login = ui_state.login_state.agree_to_tos_next_login;
                        ui_state.login_state.agree_to_tos_next_login = false;
                        // Spawn async login task
                        let handle = tokio::spawn(async move {
                            eprintln!("[LOGIN TASK] Starting login for: first='{}', last='{}'", req.first, req.last);
                            let result = login_to_secondlife(&grid_uri, &req, Some(&proxy_settings), session_udp_port).await;
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

/// Stub for LEAP bridge client connection (future expansion)
pub async fn connect_leap_bridge(host: &str, port: u16) -> std::io::Result<tokio::net::TcpStream> {
    // This is a stub for future LEAP bridge protocol implementation
    // Example: let stream = tokio::net::TcpStream::connect((host, port)).await?;
    // For now, just connect and return the stream
    tokio::net::TcpStream::connect((host, port)).await
}
