use reqwest::Client;
use std::time::Duration;
use tokio;
use std::net::TcpListener;
use std::thread;
use std::io::{Read, Write};
use std::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 FINAL PROXY VERIFICATION TEST");
    println!("Creating a test proxy server to catch requests...");
    
    // Start a simple test proxy server on a different port
    let test_port = 9999;
    let (tx, rx) = mpsc::channel();
    
    // Start test proxy server in background
    thread::spawn(move || {
        match TcpListener::bind(format!("127.0.0.1:{}", test_port)) {
            Ok(listener) => {
                println!("✅ Test proxy server listening on port {}", test_port);
                tx.send(true).unwrap();
                
                for stream in listener.incoming() {
                    match stream {
                        Ok(mut stream) => {
                            println!("🎯 PROXY REQUEST RECEIVED!");
                            
                            let mut buffer = [0; 1024];
                            match stream.read(&mut buffer) {
                                Ok(size) => {
                                    let request = String::from_utf8_lossy(&buffer[..size]);
                                    println!("📥 Request content:");
                                    println!("{}", request);
                                    
                                    // Send a basic proxy response
                                    let response = "HTTP/1.1 200 Connection established\r\n\r\n";
                                    let _ = stream.write_all(response.as_bytes());
                                }
                                Err(e) => println!("❌ Error reading from stream: {}", e),
                            }
                        }
                        Err(e) => println!("❌ Error accepting connection: {}", e),
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to bind test proxy: {}", e);
                tx.send(false).unwrap();
            }
        }
    });
    
    // Wait for server to start
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(true) => println!("✅ Test proxy server started"),
        Ok(false) => {
            println!("❌ Test proxy server failed to start");
            return Ok(());
        }
        Err(_) => {
            println!("❌ Timeout waiting for test proxy server");
            return Ok(());
        }
    }
    
    // Give server time to fully start
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    println!("\n📍 Test 1: Using our exact proxy configuration with test server");
    
    // Use the exact same proxy configuration as our app
    let proxy = reqwest::Proxy::all(&format!("http://127.0.0.1:{}", test_port))?
        .no_proxy(reqwest::NoProxy::from_string(""));
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(5))
        .user_agent("slv-rust/0.3.0")  // Same as our app
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .no_proxy() // Disable automatic proxy detection
        .build()?;
    
    println!("🔗 Making request through test proxy...");
    
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            println!("✅ Request succeeded - Status: {}", resp.status());
            println!("📄 Response: {}", resp.text().await?);
            println!("🎯 If you see 'PROXY REQUEST RECEIVED!' above, the proxy IS being used");
        }
        Err(e) => {
            println!("❌ Request failed: {}", e);
            println!("🎯 Check if you see 'PROXY REQUEST RECEIVED!' - that proves proxy usage");
        }
    }
    
    println!("\n📍 Test 2: Testing with Hippolyzer proxy directly");
    
    // Test with actual Hippolyzer proxy
    let hippo_proxy = reqwest::Proxy::all("http://127.0.0.1:9062")?
        .no_proxy(reqwest::NoProxy::from_string(""));
    
    let hippo_client = Client::builder()
        .proxy(hippo_proxy)
        .timeout(Duration::from_secs(5))
        .user_agent("proxy-verification-test/1.0")
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .no_proxy()
        .build()?;
    
    println!("🔗 Making request through Hippolyzer proxy...");
    
    match hippo_client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            println!("✅ Hippolyzer request succeeded - Status: {}", resp.status());
            println!("📄 Response: {}", resp.text().await?);
            println!("🔍 Check Hippolyzer logs for 'proxy-verification-test/1.0'");
        }
        Err(e) => {
            println!("❌ Hippolyzer request failed: {}", e);
        }
    }
    
    println!("\n🎯 DIAGNOSIS:");
    println!("- If you saw 'PROXY REQUEST RECEIVED!' → reqwest CAN use proxy");
    println!("- If Hippolyzer request succeeded but didn't appear in logs → Hippolyzer issue");
    println!("- If both failed → reqwest proxy configuration problem");
    
    // Keep server running briefly to catch any delayed requests
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    Ok(())
}