use reqwest::Client;
use std::time::Duration;
use tokio;
use std::net::{TcpStream, SocketAddr};
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Debugging proxy routing...");
    
    // Test 1: Check if we can connect to proxy directly
    println!("\n📍 Test 1: Direct TCP connection to proxy");
    test_direct_connection().await?;
    
    // Test 2: Test with a simple HTTP request (not HTTPS)
    println!("\n📍 Test 2: HTTP (not HTTPS) request through proxy");
    test_http_request().await?;
    
    // Test 3: Test with the exact same configuration as your app
    println!("\n📍 Test 3: Exact SecondLife login simulation");
    test_secondlife_login_simulation().await?;
    
    // Test 4: Check what proxy is actually being used
    println!("\n📍 Test 4: Proxy environment check");
    check_proxy_environment();
    
    Ok(())
}

async fn test_direct_connection() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_addr: SocketAddr = "127.0.0.1:9062".parse()?;
    
    println!("🔗 Attempting direct TCP connection to {}...", proxy_addr);
    
    match TcpStream::connect_timeout(&proxy_addr, Duration::from_secs(5)) {
        Ok(mut stream) => {
            println!("✅ Direct TCP connection successful!");
            
            // Try to send a simple HTTP CONNECT request
            let connect_request = "CONNECT httpbin.org:443 HTTP/1.1\r\nHost: httpbin.org:443\r\n\r\n";
            
            match stream.write_all(connect_request.as_bytes()) {
                Ok(_) => println!("✅ Sent CONNECT request to proxy"),
                Err(e) => println!("⚠️ Failed to send CONNECT request: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Direct TCP connection failed: {}", e);
            println!("🔍 This suggests Hippolyzer proxy isn't listening on 127.0.0.1:9062");
        }
    }
    
    Ok(())
}

async fn test_http_request() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Testing plain HTTP (not HTTPS) request through proxy...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("debug-proxy-test/1.0")
        .build()?;
    
    println!("📡 Sending HTTP POST to http://httpbin.org/post (plain HTTP)...");
    
    match client
        .post("http://httpbin.org/post")  // Note: HTTP not HTTPS
        .header("Content-Type", "application/json")
        .header("X-Debug-Test", "plain-http-test")
        .body(r#"{"debug": "plain_http_test"}"#)
        .send()
        .await
    {
        Ok(resp) => {
            println!("✅ Plain HTTP request successful!");
            println!("  - Status: {}", resp.status());
            println!("🔍 Check Hippolyzer for HTTP (not HTTPS) traffic to httpbin.org");
        }
        Err(e) => {
            println!("❌ Plain HTTP request failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_secondlife_login_simulation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Testing exact SecondLife login configuration...");
    
    // Use the exact same proxy configuration as your main app
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("slv-rust/0.3.0")  // Same as your app
        .proxy(proxy)
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;
    
    // Create a minimal XML-RPC payload similar to SecondLife login
    let xml_payload = r#"<?xml version="1.0"?>
<methodCall>
  <methodName>login_to_simulator</methodName>
  <params>
    <param>
      <value>
        <struct>
          <member>
            <name>first</name>
            <value><string>testuser</string></value>
          </member>
          <member>
            <name>last</name>
            <value><string>Resident</string></value>
          </member>
          <member>
            <name>passwd</name>
            <value><string>$1$testpasswordhash</string></value>
          </member>
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#;
    
    println!("📡 Sending XML-RPC to login.agni.lindenlab.com (TEST - will fail auth)...");
    println!("  - This simulates your real login request");
    println!("  - Payload size: {} bytes", xml_payload.len());
    
    let start_time = std::time::Instant::now();
    
    match client
        .post("https://login.agni.lindenlab.com/cgi-bin/login.cgi")
        .header("Content-Type", "text/xml")
        .body(xml_payload)
        .send()
        .await
    {
        Ok(resp) => {
            let elapsed = start_time.elapsed();
            println!("✅ Request to SecondLife login server successful!");
            println!("  - Status: {}", resp.status());
            println!("  - Response time: {:?}", elapsed);
            println!("  - Headers: {:?}", resp.headers());
            
            match resp.text().await {
                Ok(body) => {
                    let preview = if body.len() > 200 { 
                        format!("{}...", &body[..200]) 
                    } else { 
                        body 
                    };
                    println!("  - Response preview: {}", preview);
                }
                Err(e) => println!("  - Could not read response: {}", e),
            }
            
            println!("🎯 THIS REQUEST should appear in Hippolyzer logs!");
            println!("🔍 Look for: POST to login.agni.lindenlab.com");
        }
        Err(e) => {
            println!("❌ Request to SecondLife login server failed: {}", e);
            println!("🔍 Error details: {:?}", e);
        }
    }
    
    Ok(())
}

fn check_proxy_environment() {
    println!("🔍 Checking proxy environment variables...");
    
    let proxy_vars = [
        "http_proxy",
        "https_proxy", 
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "all_proxy",
        "ALL_PROXY",
        "no_proxy",
        "NO_PROXY"
    ];
    
    let mut found_any = false;
    for var in &proxy_vars {
        if let Ok(value) = std::env::var(var) {
            println!("  - {}: {}", var, value);
            found_any = true;
        }
    }
    
    if !found_any {
        println!("  - No proxy environment variables set");
    }
    
    println!("\n🔍 System proxy detection...");
    println!("  - macOS may have system-wide proxy settings");
    println!("  - Check System Preferences > Network > Advanced > Proxies");
    println!("  - Reqwest might be using system proxy instead of our configured proxy");
}