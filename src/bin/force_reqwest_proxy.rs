use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing ways to force reqwest to use proxy without fallback...");
    
    // Test 1: Extremely short timeout to force proxy usage
    println!("\n📍 Test 1: Short timeout to prevent fallback");
    test_short_timeout().await?;
    
    // Test 2: Disable automatic proxy detection
    println!("\n📍 Test 2: Disable system proxy detection");
    test_no_system_proxy().await?;
    
    // Test 3: Use ALL proxy instead of HTTP proxy
    println!("\n📍 Test 3: Use all() proxy instead of http() proxy");
    test_all_proxy().await?;
    
    // Test 4: Custom connector with manual proxy validation
    println!("\n📍 Test 4: Custom connector approach");
    test_custom_connector().await?;
    
    // Test 5: Environment variable method
    println!("\n📍 Test 5: Environment variable proxy");
    test_env_proxy().await?;
    
    Ok(())
}

async fn test_short_timeout() -> Result<(), Box<dyn std::error::Error>> {
    println!("⏱️ Testing with very short timeout to force proxy usage...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_millis(500))  // Very short timeout
        .connect_timeout(Duration::from_millis(500))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("short-timeout-test/1.0")
        .build()?;
    
    // Test with broken proxy first
    println!("  🔧 Testing broken proxy with short timeout...");
    let broken_proxy = reqwest::Proxy::http("http://127.0.0.1:9999")?;
    
    let broken_client = Client::builder()
        .proxy(broken_proxy)
        .timeout(Duration::from_millis(500))
        .connect_timeout(Duration::from_millis(500))
        .build()?;
    
    match broken_client.get("https://httpbin.org/ip").send().await {
        Ok(_) => {
            println!("  ❌ Broken proxy still succeeded (bad - means fallback)");
        }
        Err(e) => {
            println!("  ✅ Broken proxy failed as expected: {}", e);
            println!("      This suggests we can prevent fallback with timeout");
        }
    }
    
    // Now test working proxy
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("  📊 Working proxy result: {}", body.trim());
        }
        Err(e) => {
            println!("  ❌ Working proxy failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_no_system_proxy() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚫 Testing with system proxy detection disabled...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .no_proxy()  // Disable automatic proxy detection
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("no-system-proxy-test/1.0")
        .build()?;
    
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("  📊 No system proxy result: {}", body.trim());
        }
        Err(e) => {
            println!("  ❌ No system proxy failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_all_proxy() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Testing with all() proxy instead of http() proxy...");
    
    let proxy = reqwest::Proxy::all("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("all-proxy-test/1.0")
        .build()?;
    
    // Test broken proxy first
    let broken_proxy = reqwest::Proxy::all("http://127.0.0.1:9999")?;
    let broken_client = Client::builder()
        .proxy(broken_proxy)
        .timeout(Duration::from_secs(2))
        .build()?;
    
    println!("  🔧 Testing broken all() proxy...");
    match broken_client.get("https://httpbin.org/ip").send().await {
        Ok(_) => {
            println!("  ❌ Broken all() proxy succeeded (fallback occurred)");
        }
        Err(e) => {
            println!("  ✅ Broken all() proxy failed: {}", e);
        }
    }
    
    println!("  🔧 Testing working all() proxy...");
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("  📊 All proxy result: {}", body.trim());
        }
        Err(e) => {
            println!("  ❌ All proxy failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_custom_connector() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Testing custom connector approach...");
    
    // Try building client with very specific configuration
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?
        .no_proxy(reqwest::NoProxy::from_string(""));  // No bypass rules
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("custom-connector-test/1.0")
        .http1_only()  // Force HTTP/1.1
        .tcp_keepalive(Duration::from_secs(10))
        .build()?;
    
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("  📊 Custom connector result: {}", body.trim());
        }
        Err(e) => {
            println!("  ❌ Custom connector failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_env_proxy() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌍 Testing environment variable proxy...");
    
    // Set proxy environment variables
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9062");
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9062");
    
    // Don't set explicit proxy in client - let it use env vars
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("env-proxy-test/1.0")
        .build()?;
    
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("  📊 Env proxy result: {}", body.trim());
        }
        Err(e) => {
            println!("  ❌ Env proxy failed: {}", e);
        }
    }
    
    // Clean up env vars
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("HTTPS_PROXY");
    
    Ok(())
}