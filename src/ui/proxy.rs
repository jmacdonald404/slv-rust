use serde::{Serialize, Deserialize};
/// Stores proxy configuration for UDP and HTTP(S) traffic.
/// Default values are set for Hippolyzer compatibility.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProxySettings {
    pub enabled: bool,
    pub socks5_host: String,
    pub socks5_port: u16,
    pub http_host: String,
    pub http_port: u16,
    pub disable_cert_validation: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false, // Start disabled, will be dynamically enabled when proxy is detected
            socks5_host: "127.0.0.1".to_string(),
            socks5_port: 9061, // Hippolyzer default SOCKS5 port
            http_host: "127.0.0.1".to_string(),
            http_port: 9062, // Hippolyzer default HTTP port
            disable_cert_validation: true, // Required for Hippolyzer HTTPS
        }
    }
}

impl ProxySettings {
    /// Detect if Hippolyzer proxy is running and available
    pub fn detect_hippolyzer_proxy() -> bool {
        use std::net::TcpStream;
        use std::time::Duration;
        
        // Check if HTTP proxy port (9062) is listening
        let http_check = TcpStream::connect_timeout(
            &"127.0.0.1:9062".parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok();
        
        // Check if SOCKS5 proxy port (9061) is listening  
        let socks5_check = TcpStream::connect_timeout(
            &"127.0.0.1:9061".parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok();
        
        let proxy_detected = http_check && socks5_check;
        if proxy_detected {
            println!("✅ Hippolyzer proxy detected on ports 9061 (SOCKS5) and 9062 (HTTP) - enabling proxy");
        } else {
            println!("⚠️ Hippolyzer proxy not detected - using direct connections");
        }
        
        proxy_detected
    }
} 