use anyhow::{Context, Result};
use ureq::{Agent, AgentBuilder, Proxy};
use roxmltree;
use crate::utils::math::{Vector3, parsing as math_parsing};
use std::time::Duration;
use std::io::Read;
use super::types::*;

/// XML-RPC client for SecondLife login servers
pub struct XmlRpcClient {
    agent: Agent,
}

impl XmlRpcClient {
    pub fn new() -> Self {
        let agent = AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("slv-rust/0.3.0")
            .build();
        
        Self { agent }
    }

    /// Create a new XML-RPC client with proxy configuration
    pub fn new_with_proxy(proxy_host: &str, proxy_port: u16, disable_cert_validation: bool) -> Result<Self> {
        tracing::info!("🔧 Configuring XML-RPC client with HTTP proxy {}:{}", proxy_host, proxy_port);
        
        let proxy_url = format!("http://{}:{}", proxy_host, proxy_port);
        let proxy = Proxy::new(&proxy_url)
            .context("Failed to create HTTP proxy")?;
        
        let agent = AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("slv-rust/0.3.0")
            .proxy(proxy)
            .build();
        
        if disable_cert_validation {
            tracing::warn!("⚠️ Certificate validation disabled for proxy connection");
            tracing::warn!("⚠️ ureq 3.0.12 API limitations - proxy and timeout configuration limited");
        }
        
        tracing::info!("✅ XML-RPC client configured with ureq proxy support");
        
        Ok(Self { agent })
    }