use crate::ui::{UiState, LoginUiState, LoginProgress};
use eframe::egui;

pub fn show_menu_bar(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            show_world_menu(ui, ui_state);
            show_view_menu(ui);
            show_tools_menu(ui, ui_state);
        });
    });
}

fn show_world_menu(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.menu_button("World", |ui| {
        if ui.button("Logout").clicked() {
            ui_state.logout_requested = true;
            ui_state.login_ui_state = LoginUiState::LoginSplash;
            ui_state.login_progress = LoginProgress::Idle;
        }
    });
}

fn show_view_menu(ui: &mut egui::Ui) {
    ui.menu_button("View", |ui| {
        ui.label("Camera controls coming soon...");
    });
}

fn show_tools_menu(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.menu_button("Tools", |ui| {
        if ui.button("Preferences").clicked() {
            ui_state.login_state.prefs_modal_open = true;
        }
    });
}