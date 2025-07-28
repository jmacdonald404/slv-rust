use crate::ui::{UiState, LoginProgress};
use eframe::egui;
use egui::{RichText, Vec2};

pub fn show_login_screen(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            
            // Title
            ui.heading(RichText::new("slv-rust").size(32.0));
            ui.label("A modern SecondLife viewer built with Rust");
            ui.add_space(20.0);
            
            // Login form
            show_login_form(ui, ui_state);
            
            ui.add_space(20.0);
            
            // Preferences button
            if ui.button("Preferences").clicked() {
                ui_state.login_state.prefs_modal_open = true;
            }
        });
    });
    
    // Show preferences modal if open
    if ui_state.login_state.prefs_modal_open {
        crate::ui::main_app::preferences::show_preferences_modal(ctx, ui_state);
    }
}

fn show_login_form(ui: &mut egui::Ui, ui_state: &mut UiState) {
    egui::Frame::group(ui.style())
        .inner_margin(20.0)
        .show(ui, |ui| {
            ui.set_max_width(300.0);
            
            // Username field
            ui.label("Username:");
            ui.text_edit_singleline(&mut ui_state.login_state.username);
            ui.add_space(10.0);
            
            // Password field
            ui.label("Password:");
            let password_edit = egui::TextEdit::singleline(&mut ui_state.login_state.password)
                .password(true);
            ui.add(password_edit);
            ui.add_space(10.0);
            
            // Grid selection
            show_grid_selection(ui);
            ui.add_space(15.0);
            
            // Login button
            show_login_button(ui, ui_state);
            
            // Progress and status
            show_login_status(ui, ui_state);
        });
}

fn show_grid_selection(ui: &mut egui::Ui) {
    ui.label("Grid:");
    egui::ComboBox::from_label("")
        .selected_text("Second Life Main Grid")
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut (), (), "Second Life Main Grid");
            ui.selectable_value(&mut (), (), "Second Life Beta Grid");
            ui.selectable_value(&mut (), (), "OpenSimulator");
        });
}

fn show_login_button(ui: &mut egui::Ui, ui_state: &mut UiState) {
    let login_enabled = !ui_state.login_state.username.is_empty() 
        && !ui_state.login_state.password.is_empty()
        && matches!(ui_state.login_progress, LoginProgress::Idle);
    
    if ui.add_enabled(login_enabled, egui::Button::new("Login").min_size(Vec2::new(280.0, 30.0))).clicked() {
        super::logic::start_login(ui_state);
    }
}

fn show_login_status(ui: &mut egui::Ui, ui_state: &mut UiState) {
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
}