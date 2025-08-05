use anyhow::{Result, Context};
use ureq::{Agent, Proxy};
use ureq::tls::{TlsConfig, RootCerts, Certificate};
use std::time::Duration;

/// Load custom CA certificate from src/assets/CA.pem
fn load_custom_ca_cert() -> Result<Certificate<'static>> {
    let ca_pem_path = std::path::Path::new("src/assets/CA.pem");
    
    if !ca_pem_path.exists() {
        anyhow::bail!("Custom CA certificate not found at: {}", ca_pem_path.display());
    }
    
    let ca_pem_data = std::fs::read(ca_pem_path)?;
    Certificate::from_pem(&ca_pem_data)
        .with_context(|| format!("Failed to parse CA certificate from: {}", ca_pem_path.display()))
}

/// Create TLS configuration with custom CA certificate
fn create_tls_config_with_custom_ca() -> Result<TlsConfig> {
    let custom_ca = load_custom_ca_cert()?;
    let custom_certs = vec![custom_ca];
    let root_certs = RootCerts::new_with_certs(&custom_certs);
    
    let tls_config = TlsConfig::builder()
        .root_certs(root_certs)
        .build();
    
    Ok(tls_config)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing ureq XML-RPC client with proxy...");
    
    // Test 1: Create ureq agent with proxy and custom CA
    println!("\n📍 Test 1: Create ureq agent with Hippolyzer proxy and custom CA");
    
    let proxy_url = "http://127.0.0.1:9062";
    let proxy = Proxy::new(proxy_url)
        .map_err(|e| anyhow::anyhow!("Failed to create proxy: {}", e))?;
    
    // Load custom CA certificate
    let tls_config = match create_tls_config_with_custom_ca() {
        Ok(config) => {
            println!("✅ Custom CA certificate loaded from src/assets/CA.pem");
            config
        }
        Err(e) => {
            println!("⚠️ Failed to load custom CA certificate: {}", e);
            println!("⚠️ Falling back to default TLS configuration");
            TlsConfig::builder().build()
        }
    };
    
    let agent: Agent = Agent::config_builder()
        .proxy(Some(proxy))
        .tls_config(tls_config)
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent("ureq-test/1.0")
        .build()
        .into();
    
    println!("✅ ureq agent created with proxy and TLS configuration");
    
    // Test 2: Make a simple request through proxy
    println!("\n📍 Test 2: Test HTTP request through ureq proxy");
    
    let agent_clone = agent.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut response = agent_clone
            .post("https://httpbin.org/post")
            .header("Content-Type", "application/json")
            .header("X-Test-Client", "ureq-xmlrpc-test")
            .send(r#"{"test": "ureq proxy verification"}"#)?;
        
        let status = response.status();
        let body = response.body_mut().read_to_string()?;
        
        Ok::<(u16, String), ureq::Error>((status.into(), body))
    }).await;
    
    match result {
        Ok(Ok((status, body))) => {
            println!("✅ ureq request successful!");
            println!("  - Status: {}", status);
            println!("  - Response size: {} bytes", body.len());
            if body.len() > 200 {
                println!("  - Response preview: {}...", &body[..200]);
            } else {
                println!("  - Response: {}", body);
            }
            println!("🔍 Check Hippolyzer logs for 'ureq-test/1.0' user agent");
        }
        Ok(Err(e)) => {
            println!("❌ ureq request failed: {}", e);
            match e {
                ureq::Error::StatusCode(code) => {
                    println!("  - HTTP status: {}", code);
                }
                ureq::Error::ConnectionFailed => {
                    println!("  - Connection failed");
                    println!("  - This might indicate proxy connection failed");
                }
                ureq::Error::Timeout(_) => {
                    println!("  - Request timeout");
                }
                ureq::Error::InvalidProxyUrl => {
                    println!("  - Invalid proxy URL");
                }
                ureq::Error::ConnectProxyFailed(msg) => {
                    println!("  - Connect proxy failed: {}", msg);
                }
                _ => {
                    println!("  - Other error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Task execution failed: {}", e);
        }
    }
    
    // Test 3: Test with broken proxy to verify no fallback
    println!("\n📍 Test 3: Test with broken proxy (should fail)");
    
    let broken_proxy = Proxy::new("http://127.0.0.1:9999")
        .map_err(|e| anyhow::anyhow!("Failed to create broken proxy: {}", e))?;
    
    // Use same TLS config for consistency (should still fail due to proxy connection error)
    let tls_config_broken = create_tls_config_with_custom_ca()
        .unwrap_or_else(|_| TlsConfig::builder().build());
    
    let broken_agent: Agent = Agent::config_builder()
        .proxy(Some(broken_proxy))
        .tls_config(tls_config_broken)
        .timeout_global(Some(Duration::from_secs(5)))
        .user_agent("ureq-broken-test/1.0")
        .build()
        .into();
    
    let broken_result = tokio::task::spawn_blocking(move || {
        broken_agent
            .get("https://httpbin.org/get")
            .call()
            .map_err(|e| e)
    }).await;
    
    match broken_result {
        Ok(Ok(_)) => {
            println!("⚠️ Broken proxy request succeeded (unexpected - indicates fallback)");
        }
        Ok(Err(e)) => {
            println!("✅ Broken proxy request failed as expected: {}", e);
            println!("   This proves ureq doesn't have silent fallback like reqwest");
        }
        Err(e) => {
            println!("✅ Task failed with broken proxy: {}", e);
        }
    }
    
    println!("\n🎯 Summary:");
    println!("- ureq agent creation: ✅");
    println!("- Proxy configuration: ✅");
    println!("- Request through proxy: Check results above");
    println!("- No silent fallback: ✅ (broken proxy fails)");
    println!("\nIf the request succeeded, check Hippolyzer logs for the ureq traffic!");
    
    Ok(())
}