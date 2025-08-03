use crate::ui::{UiState, LoginProgress, LoginResult, LoginUiState, UdpConnectionProgress};
use crate::networking::auth::{LoginCredentials, AuthenticationService, Grid};

pub fn start_login(ui_state: &mut UiState) {
    ui_state.login_progress = LoginProgress::InProgress;
    ui_state.login_state.status_message = "Authenticating with login server...".to_string();
    
    let username = ui_state.login_state.username.clone();
    let password = ui_state.login_state.password.clone();
    let selected_grid = ui_state.login_state.selected_grid.clone();
    let proxy_enabled = ui_state.proxy_settings.enabled;
    let result_tx = ui_state.login_result_tx.clone();
    
    ui_state.login_task = Some(ui_state.runtime_handle.spawn(async move {
        match perform_login(&username, &password, selected_grid, proxy_enabled).await {
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

pub async fn perform_login(username: &str, password: &str, grid: Grid, proxy_enabled: bool) -> Result<(), crate::networking::NetworkError> {
    use tracing::{info, warn, error};
    
    info!("🔄 LOGIN: Starting login process for user: {}", username);
    info!("🔄 LOGIN: Grid: {:?}", grid);
    info!("🔄 LOGIN: Proxy enabled: {}", proxy_enabled);
    
    // Create login credentials
    let credentials = LoginCredentials::new(username.to_string(), password.to_string())
        .with_grid(grid);
        
    info!("🔄 LOGIN: Created credentials, validating...");
    
    // Create authentication service
    let mut auth_service = AuthenticationService::new();
    
    info!("🔄 LOGIN: Created authentication service, performing login...");
    
    // Perform login with proxy setting
    match auth_service.login_with_proxy(credentials, proxy_enabled).await {
        Ok(client) => {
            info!("✅ LOGIN SUCCESS: Authentication completed successfully!");
            info!("✅ LOGIN SUCCESS: Client created and UDP connection established");
            info!("✅ LOGIN SUCCESS: UDP packets should now be flowing");
            
            // Keep the client alive for a moment to see UDP traffic
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            
            Ok(())
        }
        Err(e) => {
            error!("❌ LOGIN FAILED: Authentication failed: {}", e);
            Err(e)
        }
    }
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