//! Proxy support module for SOCKS5 and HTTP proxies
//! 
//! Implements proxy client functionality to support tools like Hippolyzer

pub mod socks5;
pub mod http;

pub use socks5::*;
pub use http::*;

use std::net::SocketAddr;
use crate::networking::{NetworkError, NetworkResult};

/// Proxy configuration
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// SOCKS5 proxy address (e.g., 127.0.0.1:9061)
    pub socks5_addr: Option<SocketAddr>,
    /// HTTP proxy address (e.g., 127.0.0.1:9062)
    pub http_addr: Option<SocketAddr>,
    /// Username for authentication (optional)
    pub username: Option<String>,
    /// Password for authentication (optional)
    pub password: Option<String>,
    /// Path to CA certificate file for HTTPS proxy connections
    pub ca_cert_path: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            socks5_addr: None,
            http_addr: None,
            username: None,
            password: None,
            ca_cert_path: None,
        }
    }
}

impl ProxyConfig {
    /// Create a new proxy config for Hippolyzer default ports
    pub fn hippolyzer_default() -> Self {
        Self {
            socks5_addr: Some("127.0.0.1:9061".parse().unwrap()),
            http_addr: Some("127.0.0.1:9062".parse().unwrap()),
            username: None,
            password: None,
            ca_cert_path: Some("src/assets/CA.pem".to_string()),
        }
    }
    
    /// Create a new proxy config for Hippolyzer with custom CA cert path
    pub fn hippolyzer_with_ca_cert<P: AsRef<str>>(ca_cert_path: P) -> Self {
        Self {
            socks5_addr: Some("127.0.0.1:9061".parse().unwrap()),
            http_addr: Some("127.0.0.1:9062".parse().unwrap()),
            username: None,
            password: None,
            ca_cert_path: Some(ca_cert_path.as_ref().to_string()),
        }
    }
    
    /// Check if SOCKS5 proxy is enabled
    pub fn has_socks5(&self) -> bool {
        self.socks5_addr.is_some()
    }
    
    /// Check if HTTP proxy is enabled
    pub fn has_http(&self) -> bool {
        self.http_addr.is_some()
    }
}