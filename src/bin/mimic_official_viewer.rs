use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Testing exact official viewer mimicry...");
    
    // Official viewer details from your earlier example
    let official_user_agent = "Second Life Release 7.1.15 (15596336374)";
    let official_content_type = "text/xml";
    
    // The exact XML payload from your example (shortened for testing)
    let official_xml_payload = r#"<?xml version="1.0" ?>
<methodCall>
  <methodName>login_to_simulator</methodName>
  <params>
    <param>
      <value>
        <struct>
          <member>
            <name>address_size</name>
            <value>
              <int>64</int>
            </value>
          </member>
          <member>
            <name>agree_to_tos</name>
            <value>
              <int>0</int>
            </value>
          </member>
          <member>
            <name>channel</name>
            <value>
              <string>Second Life Release</string>
            </value>
          </member>
          <member>
            <name>first</name>
            <value>
              <string>testuser</string>
            </value>
          </member>
          <member>
            <name>last</name>
            <value>
              <string>Resident</string>
            </value>
          </member>
          <member>
            <name>passwd</name>
            <value>
              <string>$1$testpasswordhash</string>
            </value>
          </member>
          <member>
            <name>version</name>
            <value>
              <string>7.1.15.15596336374</string>
            </value>
          </member>
          <member>
            <name>platform</name>
            <value>
              <string>mac</string>
            </value>
          </member>
          <member>
            <name>options</name>
            <value>
              <array>
                <data>
                  <value>
                    <string>inventory-root</string>
                  </value>
                  <value>
                    <string>inventory-skeleton</string>
                  </value>
                </data>
              </array>
            </value>
          </member>
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#;
    
    println!("📋 Official viewer configuration:");
    println!("  - User-Agent: {}", official_user_agent);
    println!("  - Content-Type: {}", official_content_type);
    println!("  - Payload size: {} bytes", official_xml_payload.len());
    
    // Test 1: Our current implementation
    println!("\n📍 Test 1: Our current implementation");
    test_our_implementation().await?;
    
    // Test 2: Exact official viewer mimicry
    println!("\n📍 Test 2: Exact official viewer mimicry");
    test_official_mimicry(official_user_agent, official_content_type, official_xml_payload).await?;
    
    // Test 3: Check different HTTP versions
    println!("\n📍 Test 3: Different HTTP configurations");
    test_http_versions().await?;
    
    // Test 4: Check connection persistence
    println!("\n📍 Test 4: Connection persistence");
    test_connection_persistence().await?;
    
    Ok(())
}

async fn test_our_implementation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing our current implementation...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(45))
        .user_agent("slv-rust/0.3.0")  // Our current user agent
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;
    
    let simple_xml = r#"<?xml version="1.0"?><methodCall><methodName>test</methodName></methodCall>"#;
    
    match client
        .post("https://login.agni.lindenlab.com/cgi-bin/login.cgi")
        .header("Content-Type", "text/xml")
        .body(simple_xml)
        .send()
        .await
    {
        Ok(resp) => {
            println!("  ✅ Our implementation: Status {}", resp.status());
            println!("  🔍 Check Hippolyzer for slv-rust/0.3.0 user agent");
        }
        Err(e) => {
            println!("  ❌ Our implementation failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_official_mimicry(user_agent: &str, content_type: &str, payload: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Testing exact official viewer mimicry...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(45))
        .user_agent(user_agent)  // Exact official user agent
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;
    
    match client
        .post("https://login.agni.lindenlab.com/cgi-bin/login.cgi")
        .header("Content-Type", content_type)
        .header("Accept", "*/*")
        .header("Accept-Encoding", "deflate, gzip")
        .header("Connection", "keep-alive")
        .header("Keep-alive", "300")
        .body(payload.to_string())
        .send()
        .await
    {
        Ok(resp) => {
            println!("  ✅ Official mimicry: Status {}", resp.status());
            println!("  🔍 Check Hippolyzer for 'Second Life Release' user agent");
        }
        Err(e) => {
            println!("  ❌ Official mimicry failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_http_versions() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing different HTTP configurations...");
    
    // Test with HTTP/1.1 explicitly
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .user_agent("http-version-test/1.1")
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .http1_only()  // Force HTTP/1.1
        .build()?;
    
    match client
        .post("https://httpbin.org/post")
        .header("Content-Type", "text/xml")
        .body("<test>HTTP/1.1 only</test>")
        .send()
        .await
    {
        Ok(resp) => {
            println!("  ✅ HTTP/1.1 only: Status {}", resp.status());
        }
        Err(e) => {
            println!("  ❌ HTTP/1.1 failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_connection_persistence() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing connection persistence...");
    
    let proxy = reqwest::Proxy::http("http://127.0.0.1:9062")?;
    
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .user_agent("persistence-test/1.0")
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()?;
    
    // Make multiple requests to see connection reuse
    for i in 1..=3 {
        println!("  🔗 Request {}/3", i);
        
        match client
            .post("https://httpbin.org/post")
            .header("Content-Type", "text/xml")
            .header("Connection", "keep-alive")
            .body(format!("<test>Request {}</test>", i))
            .send()
            .await
        {
            Ok(resp) => {
                println!("    ✅ Status: {}", resp.status());
            }
            Err(e) => {
                println!("    ❌ Failed: {}", e);
            }
        }
        
        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    Ok(())
}