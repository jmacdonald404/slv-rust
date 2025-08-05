use anyhow::Result;
use ureq::{Agent, Proxy};
use std::time::Duration;

fn main() -> Result<()> {
    println!("🧪 Testing basic ureq functionality...");
    
    // Test 1: Create simple agent without proxy
    println!("📍 Test 1: Create simple ureq agent");
    let simple_agent: Agent = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent("simple-test/1.0")
        .build()
        .into();
    println!("✅ Simple agent created");
    
    // Test 2: Try creating proxy
    println!("📍 Test 2: Create proxy configuration");
    let proxy_url = "http://127.0.0.1:9062";
    let proxy = Proxy::new(proxy_url)
        .map_err(|e| anyhow::anyhow!("Failed to create proxy: {}", e))?;
    println!("✅ Proxy created");
    
    // Test 3: Create agent with proxy
    println!("📍 Test 3: Create agent with proxy");
    let proxy_agent: Agent = Agent::config_builder()
        .proxy(Some(proxy))
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent("proxy-test/1.0")
        .build()
        .into();
    println!("✅ Proxy agent created");
    
    println!("🎯 All basic ureq operations successful");
    Ok(())
}