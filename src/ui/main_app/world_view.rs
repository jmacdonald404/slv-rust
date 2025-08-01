use crate::ui::UiState;
use eframe::egui;

pub fn show_world_view(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Welcome to Second Life!");
        ui.label("3D world rendering will appear here once the rendering engine is integrated.");
        
        show_agent_info(ui, ui_state);
    });
}

fn show_agent_info(ui: &mut egui::Ui, ui_state: &mut UiState) {
    if let Some(ref agent_state) = ui_state.agent_state {
        ui.separator();
        ui.label(format!("God Level: {}", agent_state.god_level));
        ui.label(format!("Language: {}", agent_state.language));
        ui.label(format!("Hover Height: {:.2}m", agent_state.hover_height));
    }
}