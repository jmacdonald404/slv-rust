use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    println!("🧪 Testing HTTP POST through Hippolyzer proxy...");
    
    // Test configuration
    let proxy_host = "127.0.0.1";
    let proxy_port = 9062; // Hippolyzer HTTP proxy port
    let test_url = "https://httpbin.org/post"; // Public test endpoint
    
    println!("📋 Test configuration:");
    println!("  - Proxy: {}:{}", proxy_host, proxy_port);
    println!("  - Target URL: {}", test_url);
    println!("  - Disable cert validation: true (for Hippolyzer)");
    
    // Create proxy configuration
    let proxy_url = format!("http://{}:{}", proxy_host, proxy_port);
    println!("🔧 Creating proxy with URL: {}", proxy_url);
    
    let proxy = reqwest::Proxy::http(&proxy_url)?;
    
    // Create HTTP client with proxy and disabled cert validation (needed for Hippolyzer)
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("slv-rust-proxy-test/1.0")
        .build()?;
    
    println!("✅ HTTP client created with proxy configuration");
    
    // Test payload - simple JSON
    let test_payload = r#"{"test": "proxy_verification", "message": "Hello from slv-rust!", "timestamp": "2025-08-03T07:00:00Z"}"#;
    
    println!("📡 Sending HTTP POST request...");
    println!("  - Payload size: {} bytes", test_payload.len());
    println!("  - Content-Type: application/json");
    
    let start_time = std::time::Instant::now();
    
    // Send the POST request
    let response = client
        .post(test_url)
        .header("Content-Type", "application/json")
        .header("X-Test-Source", "slv-rust-proxy-test")
        .body(test_payload)
        .send()
        .await;
    
    let elapsed = start_time.elapsed();
    
    match response {
        Ok(resp) => {
            println!("✅ HTTP POST successful!");
            println!("📊 Response details:");
            println!("  - Status: {}", resp.status());
            println!("  - Response time: {:?}", elapsed);
            println!("  - Headers: {:?}", resp.headers());
            
            // Try to get the response body
            match resp.text().await {
                Ok(body) => {
                    println!("📄 Response body (first 500 chars):");
                    let preview = if body.len() > 500 {
                        format!("{}...", &body[..500])
                    } else {
                        body
                    };
                    println!("{}", preview);
                }
                Err(e) => {
                    println!("⚠️ Could not read response body: {}", e);
                }
            }
            
            println!("🎉 Test completed successfully!");
            println!("🔍 Check your Hippolyzer logs - you should see:");
            println!("  - HTTP POST to {}", test_url);
            println!("  - User-Agent: slv-rust-proxy-test/1.0");
            println!("  - X-Test-Source: slv-rust-proxy-test");
        }
        Err(e) => {
            println!("❌ HTTP POST failed!");
            println!("💥 Error: {}", e);
            println!("🔍 Possible issues:");
            println!("  - Hippolyzer proxy not running on {}:{}", proxy_host, proxy_port);
            println!("  - Proxy not accepting HTTP traffic");
            println!("  - Network connectivity issues");
            println!("  - Firewall blocking connections");
            
            return Err(e);
        }
    }
    
    // Additional test: Try a simple GET request
    println!("\n🧪 Testing HTTP GET as well...");
    let get_start = std::time::Instant::now();
    
    match client.get("https://httpbin.org/get").send().await {
        Ok(resp) => {
            let get_elapsed = get_start.elapsed();
            println!("✅ HTTP GET also successful!");
            println!("  - Status: {}", resp.status());
            println!("  - Response time: {:?}", get_elapsed);
        }
        Err(e) => {
            println!("⚠️ HTTP GET failed: {}", e);
        }
    }
    
    println!("\n🎯 Summary:");
    println!("If both requests succeeded, your Hippolyzer proxy is working correctly!");
    println!("The HTTP POST requests should appear in your Hippolyzer logs.");
    
    Ok(())
}