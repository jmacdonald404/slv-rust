use crate::ui::{UiState, LoginProgress, LoginResult, LoginUiState, UdpConnectionProgress};
use crate::networking::auth::{LoginCredentials, AuthenticationService, Grid};

pub fn start_login(ui_state: &mut UiState) {
    ui_state.login_progress = LoginProgress::InProgress;
    ui_state.login_state.status_message = "Authenticating with login server...".to_string();
    
    let username = ui_state.login_state.username.clone();
    let password = ui_state.login_state.password.clone();
    let selected_grid = ui_state.login_state.selected_grid.clone();
    let result_tx = ui_state.login_result_tx.clone();
    
    ui_state.login_task = Some(ui_state.runtime_handle.spawn(async move {
        match perform_login(&username, &password, selected_grid).await {
            Ok(_) => {
                let _ = result_tx.send(LoginResult { result: Ok(()) });
            }
            Err(e) => {
                let _ = result_tx.send(LoginResult { 
                    result: Err(e.to_string()) 
                });
            }
        }
    }));
}

pub async fn perform_login(username: &str, password: &str, grid: Grid) -> Result<(), crate::networking::NetworkError> {
    // Create login credentials
    let credentials = LoginCredentials::new(username.to_string(), password.to_string())
        .with_grid(grid)
        .with_start_location("last".to_string());
    
    // Create authentication service
    let mut auth_service = AuthenticationService::new();
    
    // Perform login
    let _client = auth_service.login(credentials).await?;
    
    // Login successful
    Ok(())
}

pub fn start_world_connection(ui_state: &mut UiState) {
    let udp_tx = ui_state.udp_connect_tx.clone();
    
    ui_state.udp_connect_task = Some(ui_state.runtime_handle.spawn(async move {
        // Simulate world loading time
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        
        // Signal that connection is complete
        let _ = udp_tx.send(());
    }));
}