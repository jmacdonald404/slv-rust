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
            enabled: false,
            socks5_host: "127.0.0.1".to_string(),
            socks5_port: 9061, // Hippolyzer default SOCKS5 port
            http_host: "127.0.0.1".to_string(),
            http_port: 9062, // Hippolyzer default HTTP port
            disable_cert_validation: true, // Required for Hippolyzer HTTPS
        }
    }
} 