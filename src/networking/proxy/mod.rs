//! Proxy support module for SOCKS5 and HTTP proxies
//! 
//! Implements proxy client functionality to support tools like Hippolyzer.
//! Supports both direct SOCKS5 implementation and WinHippoAutoProxy transparent mode.
//!
//! ## Proxy Modes
//!
//! ### Manual SOCKS5 Mode (Linux/Mac)
//! The application implements the SOCKS5 UDP protocol directly:
//! - Establishes TCP control connection to proxy
//! - Wraps UDP packets with SOCKS5 headers
//! - Sends to proxy address instead of destination
//!
//! ### WinHippoAutoProxy Mode (Windows)
//! Uses transparent proxy interception via WinHippoAutoProxy:
//! - WinHippoAutoProxy intercepts UDP sendto()/recvfrom() calls
//! - Application sends packets to original destination 
//! - WinHippoAutoProxy automatically wraps/unwraps SOCKS5 headers
//! - No manual SOCKS5 implementation needed in application
//!
//! ### Direct Mode
//! No proxy - direct UDP connections to destinations.
//!
//! ## Usage Examples
//!
//! ```rust
//! use slv_rust::networking::proxy::{ProxyConfig, ProxyMode};
//!
//! // Auto-detect proxy mode based on platform/environment
//! let config = ProxyConfig::hippolyzer_default();
//!
//! // Force a specific proxy mode
//! let config = ProxyConfig::hippolyzer_with_mode(ProxyMode::WinHippoAutoProxy);
//! ```

pub mod socks5;
pub mod http;
pub mod transparent;

pub use socks5::*;
pub use self::http::{HttpProxyClient};
pub use transparent::*;

use std::net::SocketAddr;
use crate::networking::{NetworkError, NetworkResult};
use tracing::{info, warn};

/// Proxy operating mode
#[derive(Debug, Clone, PartialEq)]
pub enum ProxyMode {
    /// No proxy - direct connection
    Direct,
    /// Manual SOCKS5 implementation (Linux/Mac)
    /// Application handles SOCKS5 protocol directly
    ManualSocks5,
    /// Transparent proxy mode (Windows with WinHippoAutoProxy)
    /// WinHippoAutoProxy intercepts UDP calls and handles SOCKS5 transparently
    WinHippoAutoProxy,
}

impl ProxyMode {
    /// Detect the appropriate proxy mode based on platform and environment
    pub fn detect() -> Self {
        // On Windows, prefer WinHippoAutoProxy mode
        #[cfg(target_os = "windows")]
        {
            // Check if Hippolyzer is running (our internal WinHippoAutoProxy implementation)
            if Self::detect_hippolyzer_running() {
                info!("🔍 Detected Hippolyzer running - using integrated WinHippoAutoProxy mode");
                return ProxyMode::WinHippoAutoProxy;
            }
            
            // Check if external WinHippoAutoProxy might be running
            if std::env::var("WINHIPPOAUTOPROXY_ACTIVE").is_ok() ||
               Self::detect_winhippoautoproxy_process() {
                info!("🔍 Detected external WinHippoAutoProxy environment - using transparent proxy mode");
                return ProxyMode::WinHippoAutoProxy;
            }
            
            // Default to manual SOCKS5 on Windows if neither is detected
            warn!("⚠️ Windows detected but Hippolyzer not found. Using manual SOCKS5 mode.");
            warn!("   For better compatibility, ensure Hippolyzer is running on ports 9061/9062");
            ProxyMode::ManualSocks5
        }
        
        // On Linux/Mac, check if Hippolyzer is running first
        #[cfg(not(target_os = "windows"))]
        {
            // Check if Hippolyzer is running (works on all platforms)
            if Self::detect_hippolyzer_running_cross_platform() {
                info!("🔍 Detected Hippolyzer running on non-Windows platform - using WinHippoAutoProxy mode");
                return ProxyMode::WinHippoAutoProxy;
            }
            
            info!("🔍 Non-Windows platform - using manual SOCKS5 proxy mode");
            ProxyMode::ManualSocks5
        }
    }
    
    /// Detect if Hippolyzer is running by checking if proxy ports are available
    #[cfg(target_os = "windows")]
    fn detect_hippolyzer_running() -> bool {
        use std::net::TcpStream;
        use std::time::Duration;
        
        // Check if Hippolyzer's SOCKS5 proxy port (9061) is listening
        let socks5_check = TcpStream::connect_timeout(
            &"127.0.0.1:9061".parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok();
        
        // Check if Hippolyzer's HTTP proxy port (9062) is listening  
        let http_check = TcpStream::connect_timeout(
            &"127.0.0.1:9062".parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok();
        
        let hippolyzer_detected = socks5_check && http_check;
        if hippolyzer_detected {
            info!("✅ Hippolyzer detected on ports 9061 (SOCKS5) and 9062 (HTTP)");
        }
        
        hippolyzer_detected
    }
    
    /// Detect if WinHippoAutoProxy process might be running
    #[cfg(target_os = "windows")]
    fn detect_winhippoautoproxy_process() -> bool {
        // Simple process detection - look for WinHippoAutoProxy-related processes
        // This is a heuristic and may not be 100% accurate
        use std::process::Command;
        
        match Command::new("tasklist")
            .args(&["/FI", "IMAGENAME eq WinHippoAutoProxy*"])
            .output() {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                output_str.contains("WinHippoAutoProxy") || output_str.contains("winhippoautoproxy")
            }
            Err(_) => false,
        }
    }
    
    /// Detect if Hippolyzer is running by checking if proxy ports are available (cross-platform)
    fn detect_hippolyzer_running_cross_platform() -> bool {
        use std::net::TcpStream;
        use std::time::Duration;
        
        // Check if Hippolyzer's SOCKS5 proxy port (9061) is listening
        let socks5_check = TcpStream::connect_timeout(
            &"127.0.0.1:9061".parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok();
        
        // Check if Hippolyzer's HTTP proxy port (9062) is listening  
        let http_check = TcpStream::connect_timeout(
            &"127.0.0.1:9062".parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok();
        
        let is_running = socks5_check && http_check;
        if is_running {
            info!("✅ Hippolyzer proxy ports detected: SOCKS5={}, HTTP={}", socks5_check, http_check);
        } else {
            info!("❌ Hippolyzer proxy ports not detected: SOCKS5={}, HTTP={}", socks5_check, http_check);
        }
        
        is_running
    }

    /// Force a specific proxy mode (for testing or manual configuration)
    pub fn force(mode: ProxyMode) -> Self {
        match mode {
            ProxyMode::WinHippoAutoProxy => {
                info!("🔧 Forcing WinHippoAutoProxy transparent proxy mode");
            }
            ProxyMode::ManualSocks5 => {
                info!("🔧 Forcing manual SOCKS5 proxy mode");
            }
            ProxyMode::Direct => {
                info!("🔧 Forcing direct connection (no proxy)");
            }
        }
        mode
    }
}

/// Proxy configuration with dual-mode support
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Proxy operating mode
    pub mode: ProxyMode,
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
            mode: ProxyMode::Direct,
            socks5_addr: None,
            http_addr: None,
            username: None,
            password: None,
            ca_cert_path: None,
        }
    }
}

impl ProxyConfig {
    /// Create a new proxy config for Hippolyzer with auto-detected mode
    pub fn hippolyzer_default() -> Self {
        let mode = ProxyMode::detect();
        Self {
            mode,
            socks5_addr: Some("127.0.0.1:9061".parse().unwrap()),
            http_addr: Some("127.0.0.1:9062".parse().unwrap()),
            username: None,
            password: None,
            ca_cert_path: Some("src/assets/CA.pem".to_string()),
        }
    }
    
    /// Create a new proxy config for Hippolyzer with custom CA cert path and auto-detected mode
    pub fn hippolyzer_with_ca_cert<P: AsRef<str>>(ca_cert_path: P) -> Self {
        let mode = ProxyMode::detect();
        Self {
            mode,
            socks5_addr: Some("127.0.0.1:9061".parse().unwrap()),
            http_addr: Some("127.0.0.1:9062".parse().unwrap()),
            username: None,
            password: None,
            ca_cert_path: Some(ca_cert_path.as_ref().to_string()),
        }
    }
    
    /// Create a new proxy config for Hippolyzer with forced proxy mode
    pub fn hippolyzer_with_mode(mode: ProxyMode) -> Self {
        let forced_mode = ProxyMode::force(mode);
        Self {
            mode: forced_mode,
            socks5_addr: Some("127.0.0.1:9061".parse().unwrap()),
            http_addr: Some("127.0.0.1:9062".parse().unwrap()),
            username: None,
            password: None,
            ca_cert_path: Some("src/assets/CA.pem".to_string()),
        }
    }
    
    /// Check if SOCKS5 proxy is enabled
    pub fn has_socks5(&self) -> bool {
        self.socks5_addr.is_some() && self.mode != ProxyMode::Direct
    }
    
    /// Check if HTTP proxy is enabled
    pub fn has_http(&self) -> bool {
        self.http_addr.is_some() && self.mode != ProxyMode::Direct
    }
    
    /// Check if this configuration requires manual SOCKS5 implementation
    pub fn requires_manual_socks5(&self) -> bool {
        self.mode == ProxyMode::ManualSocks5 && self.has_socks5()
    }
    
    /// Check if this configuration uses transparent proxy mode
    pub fn is_transparent_proxy(&self) -> bool {
        self.mode == ProxyMode::WinHippoAutoProxy
    }
    
    /// Get the target address for UDP packets based on proxy mode
    /// - WinHippoAutoProxy: Return original destination (transparent)
    /// - ManualSocks5: Return proxy address for manual handling
    /// - Direct: Return original destination
    pub fn get_target_address(&self, original_dest: SocketAddr) -> SocketAddr {
        match self.mode {
            ProxyMode::WinHippoAutoProxy | ProxyMode::Direct => {
                // In transparent mode, send to original destination
                // WinHippoAutoProxy will intercept and handle SOCKS5
                original_dest
            }
            ProxyMode::ManualSocks5 => {
                // In manual mode, we need to send to proxy address
                // and handle SOCKS5 protocol ourselves
                self.socks5_addr.unwrap_or(original_dest)
            }
        }
    }
}