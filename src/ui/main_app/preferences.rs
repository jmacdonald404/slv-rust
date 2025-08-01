use crate::ui::UiState;
use eframe::egui;
use egui::{Align2, Vec2};

pub fn show_preferences_modal(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::Window::new("Preferences")
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_width(400.0);
            
            show_graphics_settings(ui, ui_state);
            ui.separator();
            show_network_settings(ui, ui_state);
            ui.separator();
            show_proxy_settings(ui, ui_state);
            ui.separator();
            show_audio_settings(ui, ui_state);
            ui.separator();
            show_preferences_buttons(ui, ui_state);
        });
}

fn show_graphics_settings(ui: &mut egui::Ui, ui_state: &mut UiState) {
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
}

fn show_network_settings(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.heading("Network");
    ui.add(egui::Slider::new(&mut ui_state.preferences.max_bandwidth, 500..=5000).text("Max Bandwidth (KB/s)"));
    ui.add(egui::Slider::new(&mut ui_state.preferences.timeout, 10..=60).text("Timeout (seconds)"));
}

fn show_proxy_settings(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.heading("Proxy Settings (Hippolyzer Support)");
    
    ui.checkbox(&mut ui_state.proxy_settings.enabled, "Enable Proxy");
    
    if ui_state.proxy_settings.enabled {
        ui.indent("proxy_indent", |ui| {
            ui.horizontal(|ui| {
                ui.label("SOCKS5 Host:");
                ui.text_edit_singleline(&mut ui_state.proxy_settings.socks5_host);
            });
            
            ui.horizontal(|ui| {
                ui.label("SOCKS5 Port:");
                ui.add(egui::DragValue::new(&mut ui_state.proxy_settings.socks5_port)
                    .range(1..=65535)
                    .speed(1));
            });
            
            ui.horizontal(|ui| {
                ui.label("HTTP Host:");
                ui.text_edit_singleline(&mut ui_state.proxy_settings.http_host);
            });
            
            ui.horizontal(|ui| {
                ui.label("HTTP Port:");
                ui.add(egui::DragValue::new(&mut ui_state.proxy_settings.http_port)
                    .range(1..=65535)
                    .speed(1));
            });
            
            ui.checkbox(&mut ui_state.proxy_settings.disable_cert_validation, "Disable Certificate Validation");
            
            ui.separator();
            
            // Hippolyzer preset button
            if ui.button("Set Hippolyzer Defaults").clicked() {
                ui_state.proxy_settings.socks5_host = "127.0.0.1".to_string();
                ui_state.proxy_settings.socks5_port = 9061;
                ui_state.proxy_settings.http_host = "127.0.0.1".to_string();
                ui_state.proxy_settings.http_port = 9062;
                ui_state.proxy_settings.disable_cert_validation = true;
            }
            
            // Platform-specific warnings
            ui.separator();
            ui.label("⚠️ Platform Notes:");
            
            #[cfg(target_os = "windows")]
            ui.colored_label(
                egui::Color32::from_rgb(255, 165, 0),
                "Windows: Use WinHippoAutoProxy wrapper instead of these settings"
            );
            
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            ui.colored_label(
                egui::Color32::from_rgb(0, 200, 0),
                "Linux/macOS: These settings will configure SOCKS5/HTTP proxy support"
            );
            
            ui.label("💡 For asset optimization, set no_proxy env var:");
            ui.monospace("no_proxy=\"asset-cdn.glb.agni.lindenlab.com\"");
        });
    }
}

fn show_audio_settings(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.heading("Audio");
    ui.checkbox(&mut ui_state.preferences.enable_sound, "Enable Sound");
    ui.add(egui::Slider::new(&mut ui_state.preferences.volume, 0.0..=1.0).text("Master Volume"));
}

fn show_preferences_buttons(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            if let Err(e) = crate::config::settings::save_general_settings(
                &ui_state.preferences, 
                &ui_state.proxy_settings
            ) {
                eprintln!("Failed to save preferences: {}", e);
            }
            ui_state.login_state.prefs_modal_open = false;
        }
        if ui.button("Cancel").clicked() {
            ui_state.login_state.prefs_modal_open = false;
        }
    });
}