use crate::ui::{UiState, LoginProgress, LoginResult, LoginUiState, UdpConnectionProgress};

pub fn start_login(ui_state: &mut UiState) {
    ui_state.login_progress = LoginProgress::InProgress;
    ui_state.login_state.status_message = "Authenticating with login server...".to_string();
    
    let username = ui_state.login_state.username.clone();
    let password = ui_state.login_state.password.clone();
    let result_tx = ui_state.login_result_tx.clone();
    
    ui_state.login_task = Some(ui_state.runtime_handle.spawn(async move {
        match perform_login(&username, &password).await {
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

pub async fn perform_login(username: &str, password: &str) -> Result<(), crate::networking::NetworkError> {
    use crate::networking::client::{Client, ClientConfig};
    use std::net::SocketAddr;
    use uuid::Uuid;
    
    // Simulate login server authentication
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    
    // For now, just validate non-empty credentials
    if username.is_empty() || password.is_empty() {
        return Err(crate::networking::NetworkError::HandshakeTimeout);
    }
    
    // Create client config
    let config = ClientConfig {
        agent_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        ..Default::default()
    };
    
    // Create networking client
    let client = Client::new(config).await?;
    
    // Connect to simulator (simulating SL main grid)
    let simulator_address = "127.0.0.1:9000".parse::<SocketAddr>().unwrap();
    let circuit_code = 12345;
    
    client.connect(simulator_address, circuit_code).await?;
    
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