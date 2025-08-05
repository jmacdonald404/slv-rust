use anyhow::{Result, Context};
use ureq::{Agent, Proxy};
use ureq::tls::{TlsConfig, RootCerts, Certificate};
use std::time::Duration;

fn load_custom_ca_cert() -> Result<Certificate<'static>> {
    let ca_pem_path = std::path::Path::new("src/assets/CA.pem");
    
    if !ca_pem_path.exists() {
        anyhow::bail!("Custom CA certificate not found at: {}", ca_pem_path.display());
    }
    
    let ca_pem_data = std::fs::read(ca_pem_path)
        .with_context(|| format!("Failed to read CA certificate from: {}", ca_pem_path.display()))?;
    
    Certificate::from_pem(&ca_pem_data)
        .with_context(|| format!("Failed to parse CA certificate from: {}", ca_pem_path.display()))
}

fn create_tls_config_with_custom_ca() -> Result<TlsConfig> {
    let custom_ca = load_custom_ca_cert()?;
    
    // Create a vector with the custom CA certificate
    let custom_certs = vec![custom_ca];
    let root_certs = RootCerts::new_with_certs(&custom_certs);
    
    let tls_config = TlsConfig::builder()
        .root_certs(root_certs)
        .build();
    
    println!("✅ Custom CA certificate loaded and configured for TLS verification");
    
    Ok(tls_config)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing ureq 3.0.12 with custom CA certificate...");
    
    // Test 1: Load and configure custom CA certificate
    println!("\n📍 Test 1: Load custom CA certificate");
    let tls_config = create_tls_config_with_custom_ca()?;
    println!("✅ TLS configuration created with custom CA");
    
    // Test 2: Create agent with custom CA (no proxy)
    println!("\n📍 Test 2: Create agent with custom CA certificate");
    let agent: Agent = Agent::config_builder()
        .tls_config(tls_config.clone())
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent("custom-ca-test/1.0")
        .build()
        .into();
    println!("✅ Agent created with custom CA certificate");
    
    // Test 3: Create agent with custom CA and proxy
    println!("\n📍 Test 3: Create agent with custom CA certificate and proxy");
    let proxy_url = "http://127.0.0.1:9062";
    match Proxy::new(proxy_url) {
        Ok(proxy) => {
            let proxy_agent: Agent = Agent::config_builder()
                .proxy(Some(proxy))
                .tls_config(tls_config)
                .timeout_global(Some(Duration::from_secs(30)))
                .user_agent("custom-ca-proxy-test/1.0")
                .build()
                .into();
            println!("✅ Proxy agent created with custom CA certificate");
            
            // Test 4: Make a test request through the proxy with custom CA
            println!("\n📍 Test 4: Test HTTPS request with custom CA through proxy");
            let agent_clone = proxy_agent.clone();
            let result = tokio::task::spawn_blocking(move || {
                agent_clone
                    .get("https://httpbin.org/get")
                    .header("User-Agent", "custom-ca-proxy-test/1.0")
                    .call()
            }).await;
            
            match result {
                Ok(Ok(response)) => {
                    println!("✅ HTTPS request successful through proxy with custom CA");
                    println!("  - Status: {}", response.status());
                    println!("  - This proves custom CA certificate is working correctly");
                }
                Ok(Err(e)) => {
                    println!("❌ HTTPS request failed: {}", e);
                    match e {
                        ureq::Error::StatusCode(code) => {
                            println!("  - HTTP status: {}", code);
                        }
                        ureq::Error::ConnectionFailed => {
                            println!("  - Connection failed (proxy might not be running)");
                        }
                        ureq::Error::Tls(tls_err) => {
                            println!("  - TLS error: {}", tls_err);
                            println!("  - This indicates custom CA certificate verification");
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
        }
        Err(e) => {
            println!("❌ Failed to create proxy: {}", e);
        }
    }
    
    println!("\n🎯 Custom CA certificate configuration test completed");
    Ok(())
}