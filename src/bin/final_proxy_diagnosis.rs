use std::net::TcpStream;
use std::io::{Write, Read, BufRead, BufReader};
use std::time::Duration;
use tokio;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 FINAL PROXY DIAGNOSIS - Determining what's really happening...");
    
    // Test 1: Direct connection to verify Hippolyzer is actually running
    println!("\n📍 Test 1: Verify Hippolyzer proxy is responding");
    test_proxy_response().await?;
    
    // Test 2: Force a request that MUST show up in any proxy
    println!("\n📍 Test 2: Force request that must be visible");
    test_forced_visibility().await?;
    
    // Test 3: Check if reqwest is actually using the proxy
    println!("\n📍 Test 3: Verify reqwest proxy usage");
    test_reqwest_proxy_usage().await?;
    
    // Test 4: Test with a completely broken proxy to see if it fails
    println!("\n📍 Test 4: Test with broken proxy to verify proxy is required");
    test_broken_proxy().await?;
    
    Ok(())
}

async fn test_proxy_response() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Testing direct connection to Hippolyzer proxy...");
    
    let mut stream = TcpStream::connect_timeout(
        &"127.0.0.1:9062".parse()?,
        Duration::from_secs(5)
    )?;
    
    // Send a simple HTTP request to see if proxy responds
    let request = "GET http://httpbin.org/get HTTP/1.1\r\nHost: httpbin.org\r\nConnection: close\r\nUser-Agent: direct-test/1.0\r\n\r\n";
    
    stream.write_all(request.as_bytes())?;
    
    let mut response = String::new();
    let mut reader = BufReader::new(&mut stream);
    reader.read_line(&mut response)?;
    
    println!("  📥 Proxy response: {}", response.trim());
    
    if response.contains("200") {
        println!("  ✅ Hippolyzer proxy is responding to HTTP requests");
        println!("  🔍 This request should appear in Hippolyzer logs as 'direct-test/1.0'");
    } else {
        println!("  ❌ Unexpected response from proxy");
    }
    
    Ok(())
}

async fn test_forced_visibility() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Forcing a request that MUST be visible in any HTTP proxy...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("FORCE-VISIBILITY-TEST-12345")
        .build()?;
    
    // Use a plain HTTP request that should definitely show up
    println!("  📡 Making plain HTTP request (should be 100% visible)...");
    
    match client
        .post("http://httpbin.org/post")
        .header("Content-Type", "application/json")
        .header("X-FORCE-TEST", "THIS-MUST-BE-VISIBLE")
        .body(r#"{"message": "FORCE VISIBILITY TEST - IF YOU SEE THIS IN HIPPOLYZER, THE PROXY WORKS"}"#)
        .send()
        .await
    {
        Ok(resp) => {
            println!("  ✅ Forced visibility test successful - Status: {}", resp.status());
            println!("  🔍 CHECK HIPPOLYZER NOW for:");
            println!("    - POST to httpbin.org");
            println!("    - User-Agent: FORCE-VISIBILITY-TEST-12345");
            println!("    - Header: X-FORCE-TEST: THIS-MUST-BE-VISIBLE");
            println!("    - Body: FORCE VISIBILITY TEST message");
        }
        Err(e) => {
            println!("  ❌ Forced visibility test failed: {}", e);
        }
    }
    
    // Also test HTTPS to the same endpoint
    println!("\n  📡 Making HTTPS request to same endpoint...");
    
    match client
        .post("https://httpbin.org/post")
        .header("Content-Type", "application/json")
        .header("X-HTTPS-TEST", "HTTPS-VISIBILITY-TEST")
        .body(r#"{"message": "HTTPS VISIBILITY TEST"}"#)
        .send()
        .await
    {
        Ok(resp) => {
            println!("  ✅ HTTPS test successful - Status: {}", resp.status());
            println!("  🔍 CHECK HIPPOLYZER for HTTPS request (might only show CONNECT)");
        }
        Err(e) => {
            println!("  ❌ HTTPS test failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_reqwest_proxy_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing if reqwest is actually using the proxy...");
    
    // Create a client with proxy
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    let client_with_proxy = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(5))
        .user_agent("proxy-usage-test/1.0")
        .build()?;
    
    // Create a client without proxy
    let client_no_proxy = Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("no-proxy-test/1.0")
        .build()?;
    
    println!("  📡 Request without proxy...");
    let no_proxy_result = client_no_proxy.get("https://httpbin.org/ip").send().await;
    
    println!("  📡 Request with proxy...");
    let proxy_result = client_with_proxy.get("https://httpbin.org/ip").send().await;
    
    match (no_proxy_result, proxy_result) {
        (Ok(no_proxy_resp), Ok(proxy_resp)) => {
            let no_proxy_text = no_proxy_resp.text().await?;
            let proxy_text = proxy_resp.text().await?;
            
            println!("  📊 Results comparison:");
            println!("    Without proxy: {}", no_proxy_text.trim());
            println!("    With proxy:    {}", proxy_text.trim());
            
            if no_proxy_text == proxy_text {
                println!("  ⚠️ SAME IP - Proxy is either transparent or being bypassed!");
            } else {
                println!("  ✅ DIFFERENT IPs - Proxy is working correctly");
            }
        }
        (Ok(_), Err(proxy_err)) => {
            println!("  🎯 No proxy works, proxy fails - This proves proxy is being used!");
            println!("    Proxy error: {}", proxy_err);
        }
        (Err(_), Ok(_)) => {
            println!("  🎯 No proxy fails, proxy works - Proxy is fixing connectivity");
        }
        (Err(no_proxy_err), Err(proxy_err)) => {
            println!("  ❌ Both failed:");
            println!("    No proxy: {}", no_proxy_err);
            println!("    Proxy: {}", proxy_err);
        }
    }
    
    Ok(())
}

async fn test_broken_proxy() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing with deliberately broken proxy...");
    
    let broken_proxy = reqwest::Proxy::http("http://127.0.0.1:9999")?; // Wrong port
    
    let client = Client::builder()
        .proxy(broken_proxy)
        .timeout(Duration::from_secs(3))
        .user_agent("broken-proxy-test/1.0")
        .build()?;
    
    match client.get("https://httpbin.org/ip").send().await {
        Ok(resp) => {
            println!("  ⚠️ Request with broken proxy succeeded (unexpected!)");
            println!("    This means reqwest is falling back and not using proxy");
            let body = resp.text().await?;
            println!("    Response: {}", body);
        }
        Err(e) => {
            println!("  ✅ Request with broken proxy failed as expected");
            println!("    Error: {}", e);
            println!("    This proves reqwest does try to use the proxy");
        }
    }
    
    println!("\n🎯 DIAGNOSIS SUMMARY:");
    println!("If you see ONLY the HTTP request (not HTTPS) in Hippolyzer:");
    println!("  → Hippolyzer only logs HTTP content, not HTTPS tunnels");
    println!("  → Your login requests ARE going through proxy but aren't logged");
    println!("  → Solution: Look for CONNECT logs or enable HTTPS interception");
    println!();
    println!("If you see NO requests at all in Hippolyzer:");
    println!("  → The proxy configuration is wrong");
    println!("  → Reqwest is bypassing the proxy entirely");
    println!("  → Or Hippolyzer logging is misconfigured");
    
    Ok(())
}