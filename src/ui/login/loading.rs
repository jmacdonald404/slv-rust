use crate::ui::{UiState, LoginUiState, LoginProgress, UdpConnectionProgress};
use eframe::egui;

pub fn show_loading_screen(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            
            ui.heading("Connecting to Second Life...");
            ui.add_space(20.0);
            
            ui.spinner();
            ui.add_space(10.0);
            
            show_connection_status(ui, ui_state);
        });
    });
}

fn show_connection_status(ui: &mut egui::Ui, ui_state: &mut UiState) {
    match &ui_state.udp_progress {
        UdpConnectionProgress::NotStarted => {
            ui.label("Initializing connection...");
        }
        UdpConnectionProgress::Connecting => {
            ui.label("Establishing UDP connection...");
        }
        UdpConnectionProgress::Connected => {
            ui.label("Connected! Loading world...");
        }
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
}