//! Simple proxy test to verify SOCKS5 routing works

use bytes::Bytes;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, error};

use slv_rust::networking::proxy::{ProxyConfig, ProxyMode, Socks5UdpClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🧪 Starting simple SOCKS5 proxy test");

    // Test 1: Check if Hippolyzer is running
    info!("📡 Testing Hippolyzer connectivity...");
    let hippolyzer_running = test_hippolyzer_connection().await;
    if !hippolyzer_running {
        error!("❌ Hippolyzer not detected on ports 9061/9062");
        return Ok(());
    }
    info!("✅ Hippolyzer detected and running");

    // Test 2: Create SOCKS5 client
    info!("🔌 Creating SOCKS5 client...");
    let proxy_addr: SocketAddr = "127.0.0.1:9061".parse()?;
    let mut socks5_client = Socks5UdpClient::new(proxy_addr, None, None);

    // Test 3: Connect to SOCKS5 proxy
    info!("🤝 Connecting to SOCKS5 proxy...");
    match socks5_client.connect().await {
        Ok(_) => info!("✅ SOCKS5 connection established"),
        Err(e) => {
            error!("❌ SOCKS5 connection failed: {}", e);
            return Ok(());
        }
    }

    // Test 4: Send test UDP packet through proxy
    info!("📤 Sending test packet through SOCKS5 proxy...");
    let test_data = b"Hello Hippolyzer Test!";
    let test_dest: SocketAddr = "8.8.8.8:53".parse()?; // Google DNS as test destination
    
    match socks5_client.send_to(&Bytes::from_static(test_data), test_dest).await {
        Ok(_) => info!("✅ Test packet sent through SOCKS5 proxy"),
        Err(e) => {
            error!("❌ Failed to send test packet: {}", e);
        }
    }

    // Test 5: Send another packet to a known IP
    info!("📤 Sending test packet to Cloudflare DNS...");
    let cf_test_dest: SocketAddr = "1.1.1.1:53".parse()?;
    let cf_test_data = b"Cloudflare Test Packet";
    
    match socks5_client.send_to(&Bytes::from_static(cf_test_data), cf_test_dest).await {
        Ok(_) => info!("✅ Cloudflare test packet sent through SOCKS5 proxy"),
        Err(e) => {
            error!("❌ Failed to send Cloudflare test packet: {}", e);
        }
    }

    // Test 6: Try to receive a packet (will likely timeout, but should show in Hippolyzer)
    info!("📥 Attempting to receive packet (may timeout)...");
    let mut buffer = vec![0u8; 1024];
    tokio::select! {
        result = socks5_client.recv_from(&mut buffer) => {
            match result {
                Ok((len, src)) => info!("✅ Received {} bytes from {}", len, src),
                Err(e) => info!("⏰ Receive operation result: {}", e),
            }
        }
        _ = sleep(Duration::from_secs(2)) => {
            info!("⏰ Receive timeout (expected)");
        }
    }

    info!("🏁 Test completed");
    info!("📋 Check Hippolyzer GUI/logs for the test packets:");
    info!("   - Packet to 8.8.8.8:53 with 'Hello Hippolyzer Test!'");
    info!("   - Packet to 1.1.1.1:53 with 'Cloudflare Test Packet'");
    
    // Keep connection alive for a moment
    sleep(Duration::from_secs(1)).await;
    
    Ok(())
}

async fn test_hippolyzer_connection() -> bool {
    use std::net::TcpStream;
    use std::time::Duration;
    
    // Check SOCKS5 port
    let socks5_check = TcpStream::connect_timeout(
        &"127.0.0.1:9061".parse().unwrap(),
        Duration::from_millis(100)
    ).is_ok();
    
    // Check HTTP port
    let http_check = TcpStream::connect_timeout(
        &"127.0.0.1:9062".parse().unwrap(),
        Duration::from_millis(100)
    ).is_ok();
    
    info!("   SOCKS5 (9061): {}", if socks5_check { "✅" } else { "❌" });
    info!("   HTTP (9062): {}", if http_check { "✅" } else { "❌" });
    
    socks5_check && http_check
}