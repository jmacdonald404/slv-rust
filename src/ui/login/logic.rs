use crate::ui::{UiState, LoginProgress, LoginResult, LoginUiState, UdpConnectionProgress};
use crate::networking::auth::{LoginCredentials, AuthenticationService, Grid};
use crate::ui::proxy::ProxySettings;

pub fn start_login(ui_state: &mut UiState) {
    ui_state.login_progress = LoginProgress::InProgress;
    ui_state.login_state.status_message = "Authenticating with login server...".to_string();
    
    let username = ui_state.login_state.username.clone();
    let password = ui_state.login_state.password.clone();
    let selected_grid = ui_state.login_state.selected_grid.clone();
    let proxy_settings = ui_state.proxy_settings.clone();
    let result_tx = ui_state.login_result_tx.clone();
    
    ui_state.login_task = Some(ui_state.runtime_handle.spawn(async move {
        match perform_login(&username, &password, selected_grid, &proxy_settings).await {
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

pub async fn perform_login(username: &str, password: &str, grid: Grid, proxy_settings: &ProxySettings) -> Result<(), crate::networking::NetworkError> {
    use tracing::{info, warn, error};
    
    info!("🔄 LOGIN: Starting login process for user: {}", username);
    info!("🔄 LOGIN: Grid: {:?}", grid);
    
    // Dynamic proxy detection - check if Hippolyzer is running
    let mut effective_proxy_settings = proxy_settings.clone();
    if !proxy_settings.enabled {
        info!("🔍 LOGIN: Proxy disabled in settings, checking if Hippolyzer is available...");
        if ProxySettings::detect_hippolyzer_proxy() {
            info!("✅ LOGIN: Hippolyzer detected! Enabling proxy for this session");
            effective_proxy_settings.enabled = true;
        } else {
            info!("⚠️ LOGIN: No proxy detected, proceeding with direct connection");
        }
    }
    
    info!("🔄 LOGIN: Final proxy enabled: {}", effective_proxy_settings.enabled);
    
    if effective_proxy_settings.enabled {
        info!("🔄 LOGIN: Proxy configuration:");
        info!("  - SOCKS5: {}:{}", effective_proxy_settings.socks5_host, effective_proxy_settings.socks5_port);
        info!("  - HTTP: {}:{}", effective_proxy_settings.http_host, effective_proxy_settings.http_port);
        info!("  - Cert validation disabled: {}", effective_proxy_settings.disable_cert_validation);
    }
    
    // Create login credentials
    let credentials = LoginCredentials::new(username.to_string(), password.to_string())
        .with_grid(grid);
        
    info!("🔄 LOGIN: Created credentials, validating...");
    
    // Create authentication service with effective proxy configuration
    let mut auth_service = AuthenticationService::new_with_proxy(&effective_proxy_settings)?;
    
    info!("🔄 LOGIN: Created authentication service with proxy configuration, performing login...");
    
    // Perform login with effective proxy setting (using dynamic detection result)
    match auth_service.login_with_proxy(credentials, effective_proxy_settings.enabled).await {
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