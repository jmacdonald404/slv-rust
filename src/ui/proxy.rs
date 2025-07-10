/// Stores proxy configuration for UDP and HTTP(S) traffic.
#[derive(Default, Clone)]
pub struct ProxySettings {
    pub enabled: bool,
    pub socks5_host: String,
    pub socks5_port: u16,
    pub http_host: String,
    pub http_port: u16,
    pub disable_cert_validation: bool,
} 