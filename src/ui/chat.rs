// TODO: Integrate chat UI with egui
// TODO: Implement chat window, input box, and message display

use egui::Context;
use std::collections::VecDeque;
use crate::ui::UiState;
use crate::networking::protocol::messages::Message;
use std::net::SocketAddr;

pub fn show_chat_panel(ctx: &Context, chat_input: &mut String, chat_messages: &mut VecDeque<String>, udp_circuit: &Option<std::sync::Arc<tokio::sync::Mutex<crate::networking::circuit::Circuit>>>, session_info: &Option<crate::networking::session::LoginSessionInfo>) {
    egui::Window::new("Chat").show(ctx, |ui| {
        ui.label("Chat messages:");
        for msg in chat_messages.iter() {
            ui.label(msg);
        }
        ui.separator();
        let send = ui.text_edit_singleline(chat_input).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if send && !chat_input.trim().is_empty() {
            // Send chat message over network if possible
            if let (Some(circuit_mutex), Some(session_info)) = (udp_circuit, session_info) {
                let sim_addr = format!("{}:{}", session_info.sim_ip, session_info.sim_port).parse::<SocketAddr>().ok();
                if let Some(addr) = sim_addr {
                    let msg = Message::ChatFromViewer {
                        message: chat_input.clone(),
                        channel: "local".to_string(),
                    };
                    // Spawn a task to send the message asynchronously
                    let circuit_mutex_clone = circuit_mutex.clone();
                    let text = chat_input.clone();
                    tokio::spawn(async move {
                        let mut circuit = circuit_mutex_clone.lock().await;
                        if let Err(e) = circuit.send_message(&msg, &addr).await {
                            eprintln!("Error sending chat message: {}", e);
                        }
                    });
                    chat_messages.push_back(format!("You: {}", text));
                } else {
                    chat_messages.push_back("[Error: Invalid simulator address]".to_string());
                }
            } else {
                chat_messages.push_back("[Error: Not connected]".to_string());
            }
            if chat_messages.len() > 50 {
                chat_messages.pop_front();
            }
            chat_input.clear();
        }
    });
}

/// Call this from the network receive loop to append incoming chat messages
pub fn append_incoming_chat(chat_messages: &mut VecDeque<String>, sender: &str, message: &str) {
    chat_messages.push_back(format!("{}: {}", sender, message));
    if chat_messages.len() > 50 {
        chat_messages.pop_front();
    }
}
