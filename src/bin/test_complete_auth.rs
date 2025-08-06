use anyhow::Result;
use slv_rust::networking::auth::{complete_authentication, complete_authentication_with_proxy};
use tokio;
use tracing::{info, error, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("slv_rust=debug,test_complete_auth=info")
        .init();

    info!("🚀 Testing complete Second Life authentication flow");
    
    // Get credentials from environment variables
    let username = std::env::var("SL_USERNAME")
        .unwrap_or_else(|_| "testuser".to_string());
    let password = std::env::var("SL_PASSWORD")
        .unwrap_or_else(|_| "testpass".to_string());
    
    // Use Agni grid login URL by default
    let login_url = std::env::var("SL_LOGIN_URL")
        .unwrap_or_else(|_| "https://login.agni.lindenlab.com/cgi-bin/login.cgi".to_string());
    
    // Check for proxy configuration
    let proxy_host = std::env::var("PROXY_HOST").ok();
    let proxy_port = std::env::var("PROXY_PORT").ok().and_then(|p| p.parse().ok());
    
    info!("🔐 Attempting authentication:");
    info!("  Username: {}", username);  
    info!("  Password: [hidden]");
    info!("  Login URL: {}", login_url);
    
    if let (Some(host), Some(port)) = (&proxy_host, proxy_port) {
        info!("  Proxy: {}:{}", host, port);
        info!("  Note: Using custom CA certificate for proxy HTTPS interception");
    } else {
        warn!("  No proxy configured - will attempt direct connection");
        warn!("  If you see TLS errors, try with proxy: PROXY_HOST=127.0.0.1 PROXY_PORT=8080");
    }
    
    // Perform complete authentication flow
    let result = match (proxy_host.as_ref(), proxy_port) {
        (Some(host), Some(port)) => {
            complete_authentication_with_proxy(&login_url, &username, &password, Some((host.as_str(), port))).await
        }
        _ => {
            complete_authentication(&login_url, &username, &password).await
        }
    };
    
    match result {
        Ok(simulator_connection) => {
            info!("🎉 Complete authentication successful!");
            info!("✅ Connected to simulator: {}", simulator_connection.simulator_address());
            
            let login_response = simulator_connection.login_response();
            info!("👤 Agent: {} ({})", login_response.full_name(), login_response.agent_id);
            info!("🔑 Session ID: {}", login_response.session_id);
            info!("🏷️  Circuit Code: {}", login_response.circuit_code);
            
            // Keep connection alive for a moment to demonstrate it's working
            info!("⏳ Keeping connection alive for 10 seconds...");
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            
            info!("✅ Test completed successfully");
        }
        Err(e) => {
            error!("❌ Authentication failed: {}", e);
            error!("💡 Make sure to set SL_USERNAME and SL_PASSWORD environment variables");
            error!("💡 Example: SL_USERNAME=yourname SL_PASSWORD=yourpass cargo run --bin test_complete_auth");
            return Err(e);
        }
    }
    
    Ok(())
}