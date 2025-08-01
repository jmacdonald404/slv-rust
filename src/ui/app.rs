use crate::ui::{UiState, LoginUiState, LoginResult, LoginProgress, UdpConnectionProgress};
use eframe::egui;

pub fn show_main_window(ctx: &egui::Context, ui_state: &mut UiState) {
    // Handle login results and state transitions
    handle_login_results(ui_state);
    handle_connection_results(ui_state);
    
    // Show appropriate UI based on current state
    match ui_state.login_ui_state {
        LoginUiState::LoginSplash => {
            crate::ui::login::show_login_screen(ctx, ui_state);
        }
        LoginUiState::LoadingWorld => {
            crate::ui::login::show_loading_screen(ctx, ui_state);
        }
        LoginUiState::MainApp | LoginUiState::InWorld => {
            show_main_application(ctx, ui_state);
        }
    }
}

fn handle_login_results(ui_state: &mut UiState) {
    if let Ok(login_result) = ui_state.login_result_rx.try_recv() {
        match login_result.result {
            Ok(_) => {
                ui_state.login_progress = LoginProgress::Success;
                ui_state.login_ui_state = LoginUiState::LoadingWorld;
                ui_state.udp_progress = UdpConnectionProgress::Connecting;
                ui_state.login_state.status_message = "Login successful! Connecting to world...".to_string();
                
                // Start world loading process
                crate::ui::login::logic::start_world_connection(ui_state);
            }
            Err(error) => {
                ui_state.login_progress = LoginProgress::Error(error);
                ui_state.login_state.status_message = "Login failed".to_string();
            }
        }
    }
}

fn handle_connection_results(ui_state: &mut UiState) {
    if let Ok(_) = ui_state.udp_connect_rx.try_recv() {
        ui_state.udp_progress = UdpConnectionProgress::Connected;
        ui_state.login_ui_state = LoginUiState::InWorld;
    }
}

fn show_main_application(ctx: &egui::Context, ui_state: &mut UiState) {
    // Show menu bar
    crate::ui::main_app::show_menu_bar(ctx, ui_state);
    
    // Show main world view
    crate::ui::main_app::show_world_view(ctx, ui_state);
    
    // Show preferences modal if needed
    if ui_state.login_state.prefs_modal_open {
        crate::ui::main_app::show_preferences_modal(ctx, ui_state);
    }
}