use eframe::egui::Context;
use std::collections::VecDeque;
use crate::ui::UiState;
use crate::app::App;
use crate::world::ChatEvent;

/// Display the chat panel using the new channel-based communication system
pub fn show_chat_panel(ctx: &eframe::egui::Context, app: &App, chat_input: &mut String) {
    eframe::egui::Window::new("Chat")
        .default_size([400.0, 300.0])
        .show(ctx, |ui| {
            // Display recent chat messages
            ui.vertical(|ui| {
                ui.label("Chat Messages:");
                
                eframe::egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Show recent chat history from the app
                        for chat_event in app.get_recent_chat(50) {
                            let timestamp = chat_event.timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            
                            let time_str = format!("{:02}:{:02}", 
                                (timestamp / 60) % 60, 
                                timestamp % 60
                            );
                            
                            let message_text = if chat_event.is_local_chat() {
                                format!("[{}] {}: {}", time_str, chat_event.sender_name, chat_event.message)
                            } else if chat_event.is_private_message() {
                                format!("[{}] [IM] {}: {}", time_str, chat_event.sender_name, chat_event.message)
                            } else {
                                format!("[{}] [{}] {}: {}", time_str, chat_event.channel, chat_event.sender_name, chat_event.message)
                            };
                            
                            ui.label(message_text);
                        }
                    });
                
                ui.separator();
                
                // Chat input area
                ui.horizontal(|ui| {
                    let text_edit = ui.text_edit_singleline(chat_input);
                    let send_button = ui.button("Send");
                    
                    let should_send = (text_edit.lost_focus() && ui.input(|i| i.key_pressed(eframe::egui::Key::Enter))) 
                                      || send_button.clicked();
                    
                    if should_send && !chat_input.trim().is_empty() {
                        // Send chat message using the new channel system
                        app.send_chat(chat_input.clone());
                        chat_input.clear();
                    }
                });
                
                // Connection status indicator
                ui.separator();
                ui.horizontal(|ui| {
                    let status_text = match app.get_connection_status() {
                        crate::world::ConnectionStatus::Connected => "🟢 Connected",
                        crate::world::ConnectionStatus::Connecting => "🟡 Connecting...",
                        crate::world::ConnectionStatus::Handshaking => "🟡 Handshaking...",
                        crate::world::ConnectionStatus::Disconnecting => "🟠 Disconnecting...",
                        crate::world::ConnectionStatus::Disconnected => "🔴 Disconnected",
                        crate::world::ConnectionStatus::Error(ref err) => {
                            ui.colored_label(eframe::egui::Color32::RED, format!("❌ Error: {}", err));
                            return;
                        }
                    };
                    
                    let color = match app.get_connection_status() {
                        crate::world::ConnectionStatus::Connected => eframe::egui::Color32::GREEN,
                        crate::world::ConnectionStatus::Connecting | 
                        crate::world::ConnectionStatus::Handshaking => eframe::egui::Color32::YELLOW,
                        crate::world::ConnectionStatus::Disconnecting => eframe::egui::Color32::from_rgb(255, 165, 0), // Orange
                        crate::world::ConnectionStatus::Disconnected => eframe::egui::Color32::RED,
                        crate::world::ConnectionStatus::Error(_) => eframe::egui::Color32::RED,
                    };
                    
                    ui.colored_label(color, status_text);
                });
            });
        });
}

/// Call this from the network receive loop to append incoming chat messages
pub fn append_incoming_chat(chat_messages: &mut VecDeque<String>, sender: &str, message: &str) {
    chat_messages.push_back(format!("{}: {}", sender, message));
    if chat_messages.len() > 50 {
        chat_messages.pop_front();
    }
}
