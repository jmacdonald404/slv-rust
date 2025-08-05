use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing SOCKS5 proxy (port 9061) vs HTTP proxy (port 9062)...");
    
    // Test 1: Try with SOCKS5 proxy using reqwest
    println!("\n📍 Test 1: SOCKS5 proxy configuration");
    
    // Note: reqwest doesn't directly support SOCKS5, but let's try HTTP proxy to SOCKS5
    let socks5_proxy = reqwest::Proxy::http("socks5://127.0.0.1:9061");
    
    match socks5_proxy {
        Ok(proxy) => {
            let client = Client::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(10))
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .user_agent("socks5-test/1.0")
                .build()?;
            
            match client.get("https://httpbin.org/ip").send().await {
                Ok(resp) => {
                    let body = resp.text().await?;
                    println!("✅ SOCKS5 proxy request successful!");
                    println!("  - Response: {}", body);
                }
                Err(e) => {
                    println!("❌ SOCKS5 proxy request failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Could not create SOCKS5 proxy: {}", e);
        }
    }
    
    // Test 2: Compare different proxy URLs
    println!("\n📍 Test 2: Compare proxy behavior");
    
    let test_urls = [
        ("No proxy", None),
        ("HTTP proxy", Some("http://127.0.0.1:9062")),
        ("HTTPS proxy", Some("https://127.0.0.1:9062")),
    ];
    
    for (name, proxy_url) in &test_urls {
        println!("\n🔗 Testing: {}", name);
        
        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(5))
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
        
        if let Some(url) = proxy_url {
            match reqwest::Proxy::http(*url) {
                Ok(proxy) => {
                    client_builder = client_builder.proxy(proxy);
                }
                Err(e) => {
                    println!("  ❌ Failed to create proxy: {}", e);
                    continue;
                }
            }
        }
        
        let client = client_builder.build()?;
        
        match client.get("https://httpbin.org/ip").send().await {
            Ok(resp) => {
                let body = resp.text().await?;
                println!("  ✅ Success: {}", body.trim());
            }
            Err(e) => {
                println!("  ❌ Failed: {}", e);
            }
        }
    }
    
    println!("\n🎯 Analysis:");
    println!("- If all responses show the same IP, Hippolyzer HTTP proxy is transparent");
    println!("- Your SecondLife login requests ARE going through the proxy");
    println!("- But Hippolyzer might not be logging them properly");
    println!("- Check Hippolyzer for CONNECT requests or enable verbose logging");
    
    Ok(())
}