use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Strict proxy test - forcing proxy to be used...");
    
    // Create proxy with explicit configuration
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?
        .no_proxy(reqwest::NoProxy::from_string(""));  // Disable proxy bypass
    
    println!("📋 Proxy configuration:");
    println!("  - URL: http://127.0.0.1:9062");
    println!("  - No proxy bypass enabled");
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("strict-proxy-test/1.0")
        .no_proxy()  // Disable automatic proxy detection
        .build()?;
    
    println!("✅ Client built with strict proxy configuration");
    
    // Test the connection
    println!("\n📡 Testing with strict proxy configuration...");
    
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("✅ Request successful with strict proxy");
            println!("  - Response: {}", body);
            println!("🔍 Check Hippolyzer logs for this request!");
        }
        Err(e) => {
            println!("❌ Request failed with strict proxy: {}", e);
            println!("🔍 This suggests the proxy isn't working correctly");
        }
    }
    
    // Test 2: Try with HTTPS proxy configuration
    println!("\n📡 Testing with HTTPS proxy configuration...");
    
    let https_proxy = reqwest::Proxy::https("http://127.0.0.1:9062")?
        .no_proxy(reqwest::NoProxy::from_string(""));
    
    let https_client = Client::builder()
        .proxy(https_proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("strict-proxy-test-https/1.0")
        .no_proxy()
        .build()?;
    
    match https_client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("✅ HTTPS proxy request successful");
            println!("  - Response: {}", body);
        }
        Err(e) => {
            println!("❌ HTTPS proxy request failed: {}", e);
        }
    }
    
    // Test 3: Try with all_proxy configuration
    println!("\n📡 Testing with all_proxy configuration...");
    
    let all_proxy = reqwest::Proxy::all("http://127.0.0.1:9062")?
        .no_proxy(reqwest::NoProxy::from_string(""));
    
    let all_client = Client::builder()
        .proxy(all_proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("strict-proxy-test-all/1.0")
        .no_proxy()
        .build()?;
    
    match all_client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("✅ All proxy request successful");
            println!("  - Response: {}", body);
        }
        Err(e) => {
            println!("❌ All proxy request failed: {}", e);
        }
    }
    
    Ok(())
}