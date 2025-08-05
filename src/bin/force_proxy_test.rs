use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Force proxy test - deliberately breaking without proxy...");
    
    // Test 1: Request WITHOUT proxy (should work)
    println!("\n📍 Test 1: Request WITHOUT proxy (baseline)");
    let client_no_proxy = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("force-proxy-test/1.0")
        .build()?;
    
    match client_no_proxy.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("✅ Request without proxy successful");
            println!("  - Response: {}", body);
        }
        Err(e) => {
            println!("❌ Request without proxy failed: {}", e);
        }
    }
    
    // Test 2: Request WITH proxy to a BROKEN proxy port (should fail)
    println!("\n📍 Test 2: Request with BROKEN proxy (should fail)");
    let broken_proxy = reqwest::Proxy::http("http://127.0.0.1:9999")?; // Wrong port
    
    let client_broken_proxy = Client::builder()
        .proxy(broken_proxy)
        .timeout(Duration::from_secs(5))
        .user_agent("force-proxy-test/1.0")
        .build()?;
    
    match client_broken_proxy.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            println!("⚠️ Request with broken proxy succeeded (unexpected!)");
            let body = resp.text().await?;
            println!("  - Response: {}", body);
        }
        Err(e) => {
            println!("✅ Request with broken proxy failed as expected: {}", e);
        }
    }
    
    // Test 3: Request WITH Hippolyzer proxy (should work if proxy is working)
    println!("\n📍 Test 3: Request with Hippolyzer proxy");
    let hippolyzer_proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client_hippolyzer = Client::builder()
        .proxy(hippolyzer_proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("force-proxy-test/1.0")
        .build()?;
    
    match client_hippolyzer.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            let body = resp.text().await?;
            println!("✅ Request with Hippolyzer proxy successful");
            println!("  - Response: {}", body);
            println!("🔍 This request should appear in Hippolyzer logs!");
        }
        Err(e) => {
            println!("❌ Request with Hippolyzer proxy failed: {}", e);
        }
    }
    
    // Test 4: Check if the IP changes between no-proxy and proxy requests
    println!("\n📍 Test 4: IP comparison test");
    
    // Get IP without proxy
    let no_proxy_resp = client_no_proxy.get("https://httpbin.org/ip").send().await?;
    let no_proxy_ip = no_proxy_resp.text().await?;
    
    // Get IP with proxy
    let proxy_resp = client_hippolyzer.get("https://httpbin.org/ip").send().await?;
    let proxy_ip = proxy_resp.text().await?;
    
    println!("IP without proxy: {}", no_proxy_ip.trim());
    println!("IP with proxy:    {}", proxy_ip.trim());
    
    if no_proxy_ip == proxy_ip {
        println!("⚠️ IPs are the same - proxy might not be working or might be transparent");
    } else {
        println!("✅ IPs are different - proxy is definitely working");
    }
    
    Ok(())
}