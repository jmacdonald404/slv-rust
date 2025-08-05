use reqwest::Client;
use std::time::Duration;
use tokio;
use std::net::TcpStream;
use std::io::{Write, BufRead, BufReader};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing if reqwest properly handles HTTPS through HTTP proxy...");
    
    // Test 1: Manual CONNECT to verify proxy supports it
    println!("\n📍 Test 1: Manual CONNECT test");
    test_manual_connect().await?;
    
    // Test 2: Test with verbose reqwest client
    println!("\n📍 Test 2: Reqwest with detailed error handling");
    test_reqwest_verbose().await?;
    
    // Test 3: Test with different proxy schemes
    println!("\n📍 Test 3: Different proxy scheme configurations");
    test_proxy_schemes().await?;
    
    // Test 4: Check if reqwest is actually sending CONNECT
    println!("\n📍 Test 4: Check reqwest proxy behavior");
    test_reqwest_proxy_behavior().await?;
    
    Ok(())
}

async fn test_manual_connect() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Testing manual CONNECT to verify proxy works...");
    
    let mut stream = TcpStream::connect("127.0.0.1:9062")?;
    let connect_request = "CONNECT httpbin.org:443 HTTP/1.1\r\nHost: httpbin.org:443\r\n\r\n";
    
    stream.write_all(connect_request.as_bytes())?;
    
    let mut reader = BufReader::new(&mut stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    
    if response.contains("200") {
        println!("✅ Manual CONNECT successful: {}", response.trim());
    } else {
        println!("❌ Manual CONNECT failed: {}", response.trim());
    }
    
    Ok(())
}

async fn test_reqwest_verbose() -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Testing reqwest with verbose error handling...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("reqwest-https-test/1.0")
        .build()?;
    
    println!("🔗 Attempting HTTPS request through reqwest...");
    
    match client.get("https://httpbin.org/get").send().await {
        Ok(resp) => {
            println!("✅ Reqwest HTTPS request successful!");
            println!("  - Status: {}", resp.status());
            println!("  - Headers: {:?}", resp.headers().get("server"));
            
            // Check if we can read response
            match resp.text().await {
                Ok(body) => {
                    if body.len() > 100 {
                        println!("  - Response preview: {}...", &body[..100]);
                    } else {
                        println!("  - Response: {}", body);
                    }
                }
                Err(e) => println!("  - Could not read response body: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Reqwest HTTPS request failed!");
            println!("  - Error: {}", e);
            println!("  - Error source: {:?}", e.source());
            
            // Check if it's a proxy-related error
            if e.to_string().contains("proxy") {
                println!("  - This is a PROXY-related error");
            } else if e.to_string().contains("connect") {
                println!("  - This is a CONNECTION error");
            } else if e.to_string().contains("timeout") {
                println!("  - This is a TIMEOUT error");
            } else {
                println!("  - This is some other error type");
            }
        }
    }
    
    Ok(())
}

async fn test_proxy_schemes() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing different proxy configuration schemes...");
    
    let schemes = [
        ("http://127.0.0.1:9062", "HTTP scheme"),
        ("https://127.0.0.1:9062", "HTTPS scheme"),
    ];
    
    for (proxy_url, description) in &schemes {
        println!("\n🔗 Testing {} ({})", description, proxy_url);
        
        match reqwest::Proxy::http(*proxy_url) {
            Ok(proxy) => {
                let client = Client::builder()
                    .proxy(proxy)
                    .timeout(Duration::from_secs(5))
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()?;
                
                match client.get("https://httpbin.org/ip").send().await {
                    Ok(resp) => {
                        println!("  ✅ Success with {} - Status: {}", description, resp.status());
                    }
                    Err(e) => {
                        println!("  ❌ Failed with {}: {}", description, e);
                    }
                }
            }
            Err(e) => {
                println!("  ❌ Could not create proxy with {}: {}", description, e);
            }
        }
    }
    
    Ok(())
}

async fn test_reqwest_proxy_behavior() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Checking if reqwest properly uses proxy for HTTPS...");
    
    // Test without proxy first
    let client_no_proxy = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    
    let no_proxy_result = client_no_proxy.get("https://httpbin.org/ip").send().await;
    
    // Test with proxy
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    let client_with_proxy = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;
    
    let proxy_result = client_with_proxy.get("https://httpbin.org/ip").send().await;
    
    match (no_proxy_result, proxy_result) {
        (Ok(no_proxy_resp), Ok(proxy_resp)) => {
            let no_proxy_text = no_proxy_resp.text().await?;
            let proxy_text = proxy_resp.text().await?;
            
            println!("✅ Both requests succeeded");
            println!("  - Without proxy: {}", no_proxy_text.trim());
            println!("  - With proxy:    {}", proxy_text.trim());
            
            if no_proxy_text == proxy_text {
                println!("  ⚠️ Same response - proxy might not be working or is transparent");
            } else {
                println!("  ✅ Different responses - proxy is definitely working");
            }
        }
        (Ok(_), Err(proxy_err)) => {
            println!("✅ No proxy works, ❌ Proxy fails");
            println!("  - Proxy error: {}", proxy_err);
            println!("  - This suggests reqwest can't properly use the HTTP proxy for HTTPS");
        }
        (Err(no_proxy_err), Ok(_)) => {
            println!("❌ No proxy fails, ✅ Proxy works");
            println!("  - No proxy error: {}", no_proxy_err);
            println!("  - This is unusual - proxy might be fixing connectivity issues");
        }
        (Err(no_proxy_err), Err(proxy_err)) => {
            println!("❌ Both requests failed");
            println!("  - No proxy error: {}", no_proxy_err);
            println!("  - Proxy error: {}", proxy_err);
        }
    }
    
    Ok(())
}