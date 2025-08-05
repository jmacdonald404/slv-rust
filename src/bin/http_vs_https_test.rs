use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing HTTP vs HTTPS proxy behavior...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("http-vs-https-test/1.0")
        .build()?;
    
    println!("📡 Test 1: Plain HTTP request (should appear in Hippolyzer)");
    println!("Making request to: http://httpbin.org/post");
    
    match client
        .post("http://httpbin.org/post")
        .header("Content-Type", "application/json")
        .header("X-Test-Type", "plain-http")
        .body(r#"{"test": "plain_http_request"}"#)
        .send()
        .await
    {
        Ok(resp) => {
            println!("✅ HTTP request successful - Status: {}", resp.status());
            println!("🔍 CHECK HIPPOLYZER: Should see POST to http://httpbin.org/post");
        }
        Err(e) => {
            println!("❌ HTTP request failed: {}", e);
        }
    }
    
    println!("\n📡 Test 2: HTTPS request (might not appear in Hippolyzer details)");
    println!("Making request to: https://httpbin.org/post");
    
    match client
        .post("https://httpbin.org/post")
        .header("Content-Type", "application/json")
        .header("X-Test-Type", "https")
        .body(r#"{"test": "https_request"}"#)
        .send()
        .await
    {
        Ok(resp) => {
            println!("✅ HTTPS request successful - Status: {}", resp.status());
            println!("🔍 CHECK HIPPOLYZER: Might only see CONNECT to httpbin.org:443");
        }
        Err(e) => {
            println!("❌ HTTPS request failed: {}", e);
        }
    }
    
    println!("\n📡 Test 3: SecondLife login server simulation");
    println!("Making request to: https://login.agni.lindenlab.com/cgi-bin/login.cgi");
    
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
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#;
    
    match client
        .post("https://login.agni.lindenlab.com/cgi-bin/login.cgi")
        .header("Content-Type", "text/xml")
        .header("User-Agent", "slv-rust/0.3.0")
        .body(xml_payload)
        .send()
        .await
    {
        Ok(resp) => {
            println!("✅ SecondLife request successful - Status: {}", resp.status());
            println!("🔍 CHECK HIPPOLYZER: Might only see CONNECT to login.agni.lindenlab.com:443");
            println!("🔍 The actual XML-RPC content might not be visible");
        }
        Err(e) => {
            println!("❌ SecondLife request failed: {}", e);
        }
    }
    
    println!("\n🎯 Summary:");
    println!("- HTTP requests: Full content visible in proxy logs");
    println!("- HTTPS requests: Only CONNECT tunnel visible, content encrypted");
    println!("- Your SecondLife login uses HTTPS, so Hippolyzer might only show the connection,");
    println!("  not the actual XML-RPC request content");
    println!("\n🔍 In Hippolyzer, look for:");
    println!("- HTTP: Full POST request to httpbin.org");
    println!("- HTTPS: CONNECT requests to httpbin.org:443 and login.agni.lindenlab.com:443");
    
    Ok(())
}