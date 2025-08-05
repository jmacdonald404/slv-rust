use std::net::TcpStream;
use std::io::{Write, Read, BufRead, BufReader};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing HTTPS CONNECT support on Hippolyzer proxy...");
    
    // Connect to Hippolyzer HTTP proxy
    let proxy_addr = "127.0.0.1:9062";
    println!("📡 Connecting to proxy at {}...", proxy_addr);
    
    let mut stream = TcpStream::connect_timeout(
        &proxy_addr.parse()?,
        Duration::from_secs(5)
    )?;
    
    println!("✅ Connected to proxy");
    
    // Send HTTP CONNECT request for HTTPS
    let connect_request = "CONNECT httpbin.org:443 HTTP/1.1\r\nHost: httpbin.org:443\r\nProxy-Connection: keep-alive\r\n\r\n";
    
    println!("📤 Sending CONNECT request:");
    println!("{}", connect_request.trim());
    
    stream.write_all(connect_request.as_bytes())?;
    
    // Read the response
    let mut reader = BufReader::new(&mut stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;
    
    println!("📥 Proxy response: {}", response_line.trim());
    
    if response_line.contains("200") {
        println!("✅ CONNECT successful - Hippolyzer supports HTTPS tunneling");
        println!("🔍 This means HTTPS requests should appear in Hippolyzer as CONNECT requests");
    } else if response_line.contains("407") {
        println!("🔐 Proxy requires authentication");
    } else if response_line.contains("405") || response_line.contains("501") {
        println!("❌ CONNECT method not supported - Hippolyzer doesn't support HTTPS tunneling");
        println!("🔍 This explains why you don't see HTTPS requests!");
    } else {
        println!("⚠️ Unexpected response: {}", response_line);
    }
    
    // Read any additional response headers
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        println!("   {}", line.trim());
    }
    
    println!("\n🎯 Diagnosis:");
    if response_line.contains("200") {
        println!("- Hippolyzer DOES support HTTPS tunneling");
        println!("- Check Hippolyzer settings for HTTPS logging");
        println!("- Look for CONNECT requests in a different tab/view");
    } else {
        println!("- Hippolyzer does NOT support HTTPS tunneling on port 9062");
        println!("- HTTPS requests are failing over to direct connection");
        println!("- This is why you don't see SecondLife login requests");
        println!("- Solution: Configure Hippolyzer for HTTPS, or use SOCKS5 proxy instead");
    }
    
    Ok(())
}