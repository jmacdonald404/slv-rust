use crate::ui::{UiState, LoginProgress};
use crate::ui::components::{FormField, StyledButton, ButtonStyle, StatusMessage, StatusLevel, form_section};
use crate::networking::auth::{Grid, available_grids};
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
    form_section(ui, 300.0, |ui| {
        // Username field with validation
        let username_error = validate_username(&ui_state.login_state.username);
        let mut username_field = FormField::new("Username:", &mut ui_state.login_state.username)
            .placeholder("FirstName, FirstName.LastName, or FirstName LastName");
        
        if let Some(error) = username_error {
            username_field = username_field.validation_error(error);
        }
        username_field.show(ui);
        
        // Password field with validation
        let password_error = validate_password(&ui_state.login_state.password);
        let mut password_field = FormField::new("Password:", &mut ui_state.login_state.password)
            .password();
        
        if let Some(error) = password_error {
            password_field = password_field.validation_error(error);
        }
        password_field.show(ui);
        
        // Grid selection
        show_grid_selection(ui, ui_state);
        ui.add_space(15.0);
        
        // Login button
        show_login_button(ui, ui_state);
        
        // Progress and status
        show_login_status(ui, ui_state);
    });
}

fn show_grid_selection(ui: &mut egui::Ui, ui_state: &mut UiState) {
    ui.label("Grid:");
    
    let grids = available_grids();
    let selected_text = ui_state.login_state.selected_grid.name();
    let previous_grid = ui_state.login_state.selected_grid.clone();
    
    egui::ComboBox::from_label("")
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            for grid in grids {
                ui.selectable_value(&mut ui_state.login_state.selected_grid, grid.clone(), grid.name());
            }
        });
    
    // If the grid selection changed, load credentials for the new grid
    if ui_state.login_state.selected_grid != previous_grid {
        let selected_grid = ui_state.login_state.selected_grid.clone();
        ui_state.login_state.load_credentials_for_grid(&selected_grid);
    }
    
    ui.add_space(10.0);
}

fn show_login_button(ui: &mut egui::Ui, ui_state: &mut UiState) {
    let form_valid = validate_username(&ui_state.login_state.username).is_none()
        && validate_password(&ui_state.login_state.password).is_none()
        && !ui_state.login_state.username.is_empty()
        && !ui_state.login_state.password.is_empty();
    
    let login_enabled = form_valid && matches!(ui_state.login_progress, LoginProgress::Idle);
    
    if StyledButton::new("Login")
        .style(ButtonStyle::Primary)
        .min_size(Vec2::new(280.0, 30.0))
        .enabled(login_enabled)
        .show(ui)
        .clicked() 
    {
        super::logic::start_login(ui_state);
    }
}

fn show_login_status(ui: &mut egui::Ui, ui_state: &mut UiState) {
    match &ui_state.login_progress {
        LoginProgress::InProgress => {
            ui.add_space(10.0);
            crate::ui::components::show_spinner_with_text(ui, "Logging in...");
        }
        LoginProgress::Error(error) => {
            ui.add_space(10.0);
            StatusMessage::error(format!("Error: {}", error)).show(ui);
        }
        LoginProgress::Success => {
            ui.add_space(10.0);
            StatusMessage::success("Login successful!").show(ui);
        }
        _ => {}
    }
    
    // Status message
    if !ui_state.login_state.status_message.is_empty() {
        ui.add_space(10.0);
        StatusMessage::info(&ui_state.login_state.status_message).show(ui);
    }
}

fn validate_username(username: &str) -> Option<&'static str> {
    if username.trim().is_empty() {
        return None; // Don't show error for empty field
    }
    
    let trimmed = username.trim();
    
    // Check for valid characters (letters, numbers, spaces, periods)
    if !trimmed.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '.') {
        return Some("Username can only contain letters, numbers, spaces, and periods");
    }
    
    // Don't allow multiple consecutive spaces or periods
    if trimmed.contains("  ") || trimmed.contains("..") {
        return Some("Username cannot contain consecutive spaces or periods");
    }
    
    // Don't allow starting or ending with space or period
    if trimmed.starts_with(' ') || trimmed.ends_with(' ') || 
       trimmed.starts_with('.') || trimmed.ends_with('.') {
        return Some("Username cannot start or end with spaces or periods");
    }
    
    // All formats are valid: firstname, firstname.lastname, firstname lastname
    None
}

fn validate_password(password: &str) -> Option<&'static str> {
    if password.trim().is_empty() {
        return None; // Don't show error for empty field
    }
    
    if password.len() < 4 {
        return Some("Password must be at least 4 characters");
    }
    
    None
}