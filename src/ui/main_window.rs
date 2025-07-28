use crate::ui::{UiState, LoginUiState, LoginProgress, LoginResult, UdpConnectionProgress};
use eframe::egui;
use egui::{RichText, Ui, Align2, Vec2};
use crate::ui::AgentState;

pub fn show_main_window(ctx: &egui::Context, ui_state: &mut UiState) {
    match ui_state.login_ui_state {
        LoginUiState::LoginSplash => show_login_screen(ctx, ui_state),
        LoginUiState::LoadingWorld => show_loading_screen(ctx, ui_state),
        LoginUiState::MainApp | LoginUiState::InWorld => show_main_app(ctx, ui_state),
    }
}

fn show_login_screen(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            
            // Title
            ui.heading(RichText::new("slv-rust").size(32.0));
            ui.label("A modern SecondLife viewer built with Rust");
            ui.add_space(20.0);
            
            // Login form
            egui::Frame::group(ui.style())
                .inner_margin(20.0)
                .show(ui, |ui| {
                    ui.set_max_width(300.0);
                    
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut ui_state.login_state.username);
                    ui.add_space(10.0);
                    
                    ui.label("Password:");
                    let password_edit = egui::TextEdit::singleline(&mut ui_state.login_state.password)
                        .password(true);
                    ui.add(password_edit);
                    ui.add_space(10.0);
                    
                    // Grid selection dropdown
                    ui.label("Grid:");
                    egui::ComboBox::from_label("")
                        .selected_text("Second Life Main Grid")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut (), (), "Second Life Main Grid");
                            ui.selectable_value(&mut (), (), "Second Life Beta Grid");
                            ui.selectable_value(&mut (), (), "OpenSimulator");
                        });
                    ui.add_space(15.0);
                    
                    // Login button
                    let login_enabled = !ui_state.login_state.username.is_empty() 
                        && !ui_state.login_state.password.is_empty()
                        && matches!(ui_state.login_progress, LoginProgress::Idle);
                    
                    if ui.add_enabled(login_enabled, egui::Button::new("Login").min_size(Vec2::new(280.0, 30.0))).clicked() {
                        start_login(ui_state);
                    }
                    
                    // Login progress
                    match &ui_state.login_progress {
                        LoginProgress::InProgress => {
                            ui.add_space(10.0);
                            ui.spinner();
                            ui.label("Logging in...");
                        }
                        LoginProgress::Error(error) => {
                            ui.add_space(10.0);
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                        }
                        _ => {}
                    }
                    
                    // Status message
                    if !ui_state.login_state.status_message.is_empty() {
                        ui.add_space(10.0);
                        ui.label(&ui_state.login_state.status_message);
                    }
                });
            
            ui.add_space(20.0);
            
            // Preferences button
            if ui.button("Preferences").clicked() {
                ui_state.login_state.prefs_modal_open = true;
            }
        });
    });
    
    // Show preferences modal if open
    if ui_state.login_state.prefs_modal_open {
        show_preferences_modal(ctx, ui_state);
    }
}

fn show_loading_screen(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            
            ui.heading("Connecting to Second Life...");
            ui.add_space(20.0);
            
            ui.spinner();
            ui.add_space(10.0);
            
            match &ui_state.udp_progress {
                UdpConnectionProgress::NotStarted => { ui.label("Initializing connection..."); }
                UdpConnectionProgress::Connecting => { ui.label("Establishing UDP connection..."); }
                UdpConnectionProgress::Connected => { ui.label("Connected! Loading world..."); }
                UdpConnectionProgress::Error(err) => {
                    ui.colored_label(egui::Color32::RED, format!("Connection failed: {}", err));
                    ui.add_space(10.0);
                    if ui.button("Back to Login").clicked() {
                        ui_state.login_ui_state = LoginUiState::LoginSplash;
                        ui_state.login_progress = LoginProgress::Idle;
                        ui_state.udp_progress = UdpConnectionProgress::NotStarted;
                    }
                }
            }
        });
    });
}

fn show_main_app(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("World", |ui| {
                if ui.button("Logout").clicked() {
                    ui_state.logout_requested = true;
                    ui_state.login_ui_state = LoginUiState::LoginSplash;
                    ui_state.login_progress = LoginProgress::Idle;
                }
            });
            
            ui.menu_button("View", |ui| {
                ui.label("Camera controls coming soon...");
            });
            
            ui.menu_button("Tools", |ui| {
                if ui.button("Preferences").clicked() {
                    ui_state.login_state.prefs_modal_open = true;
                }
            });
        });
    });
    
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Welcome to Second Life!");
        ui.label("3D world rendering will appear here once the rendering engine is integrated.");
        
        if let Some(ref agent_state) = ui_state.agent_state {
            ui.separator();
            ui.label(format!("God Level: {}", agent_state.god_level));
            ui.label(format!("Language: {}", agent_state.language));
            ui.label(format!("Hover Height: {:.2}m", agent_state.hover_height));
        }
    });
    
    // Show preferences modal if open
    if ui_state.login_state.prefs_modal_open {
        show_preferences_modal(ctx, ui_state);
    }
}

fn show_preferences_modal(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::Window::new("Preferences")
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_width(400.0);
            
            ui.heading("Graphics");
            ui.horizontal(|ui| {
                ui.label("Graphics API:");
                egui::ComboBox::from_label("")
                    .selected_text(&ui_state.preferences.graphics_api)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut ui_state.preferences.graphics_api, "vulkan".to_string(), "Vulkan");
                        ui.selectable_value(&mut ui_state.preferences.graphics_api, "opengl".to_string(), "OpenGL");
                        ui.selectable_value(&mut ui_state.preferences.graphics_api, "dx12".to_string(), "DirectX 12");
                        ui.selectable_value(&mut ui_state.preferences.graphics_api, "metal".to_string(), "Metal");
                    });
            });
            
            ui.checkbox(&mut ui_state.preferences.vsync, "Enable VSync");
            ui.add(egui::Slider::new(&mut ui_state.preferences.render_distance, 64..=512).text("Render Distance (m)"));
            
            ui.separator();
            ui.heading("Network");
            ui.add(egui::Slider::new(&mut ui_state.preferences.max_bandwidth, 500..=5000).text("Max Bandwidth (KB/s)"));
            ui.add(egui::Slider::new(&mut ui_state.preferences.timeout, 10..=60).text("Timeout (seconds)"));
            
            ui.separator();
            ui.heading("Audio");
            ui.checkbox(&mut ui_state.preferences.enable_sound, "Enable Sound");
            ui.add(egui::Slider::new(&mut ui_state.preferences.volume, 0.0..=1.0).text("Master Volume"));
            
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    // TODO: Save preferences to file
                    ui_state.login_state.prefs_modal_open = false;
                }
                if ui.button("Cancel").clicked() {
                    ui_state.login_state.prefs_modal_open = false;
                }
            });
        });
}

fn start_login(ui_state: &mut UiState) {
    ui_state.login_progress = LoginProgress::InProgress;
    ui_state.login_state.status_message = "Authenticating with login server...".to_string();
    
    // TODO: Integrate with networking layer
    // For now, simulate login process
    let username = ui_state.login_state.username.clone();
    let password = ui_state.login_state.password.clone();
    let result_tx = ui_state.login_result_tx.clone();
    
    ui_state.login_task = Some(ui_state.runtime_handle.spawn(async move {
        // Simulate authentication delay
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        
        // TODO: Replace with actual login logic
        if !username.is_empty() && !password.is_empty() {
            let _ = result_tx.send(LoginResult { result: Ok(()) });
        } else {
            let _ = result_tx.send(LoginResult { 
                result: Err("Invalid credentials".to_string()) 
            });
        }
    }));
}