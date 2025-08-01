//! HTTP proxy client implementation
//! 
//! Provides HTTP proxy support for Second Life HTTP requests through tools like Hippolyzer

use crate::networking::{NetworkError, NetworkResult};
use std::net::SocketAddr;
use std::path::Path;
use reqwest::{Client, Proxy, RequestBuilder, Certificate};
use tracing::{debug, info, warn};

/// HTTP proxy client wrapper
#[derive(Clone)]
pub struct HttpProxyClient {
    /// Proxy server address  
    proxy_addr: SocketAddr,
    /// Reqwest client with proxy configuration
    client: Client,
    /// Authentication credentials
    username: Option<String>,
    password: Option<String>,
}

impl HttpProxyClient {
    /// Create a new HTTP proxy client
    pub fn new(
        proxy_addr: SocketAddr, 
        username: Option<String>, 
        password: Option<String>
    ) -> NetworkResult<Self> {
        Self::new_with_ca_cert(proxy_addr, username, password, None)
    }
    
    /// Create a new HTTP proxy client with custom CA certificate
    pub fn new_with_ca_cert(
        proxy_addr: SocketAddr,
        username: Option<String>,
        password: Option<String>, 
        ca_cert_path: Option<String>,
    ) -> NetworkResult<Self> {
        // Create proxy configuration
        let proxy_url = format!("http://{}", proxy_addr);
        let mut proxy = Proxy::http(&proxy_url)
            .map_err(|e| NetworkError::Transport {
                reason: format!("Failed to create HTTP proxy: {}", e)
            })?;
        
        // Add authentication if provided
        if let (Some(username), Some(password)) = (&username, &password) {
            proxy = proxy.basic_auth(username, password);
        }
        
        // Create HTTP client builder with proxy
        let mut client_builder = Client::builder().proxy(proxy);
        
        // Load CA certificate if provided
        if let Some(ca_path) = &ca_cert_path {
            match Self::load_ca_certificate(ca_path) {
                Ok(cert) => {
                    info!("Loaded CA certificate from {}", ca_path);
                    client_builder = client_builder.add_root_certificate(cert);
                }
                Err(e) => {
                    warn!("Failed to load CA certificate from {}: {}. Using danger_accept_invalid_certs instead.", ca_path, e);
                    // Fall back to accepting invalid certs if CA loading fails
                    client_builder = client_builder.danger_accept_invalid_certs(true);
                }
            }
        } else {
            // No CA cert provided, accept invalid certs (required for Hippolyzer's self-signed certs)
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }
        
        let client = client_builder
            .build()
            .map_err(|e| NetworkError::Transport {
                reason: format!("Failed to create HTTP client: {}", e)
            })?;
        
        info!("HTTP proxy client configured for {}", proxy_addr);
        if ca_cert_path.is_some() {
            info!("CA certificate path: {:?}", ca_cert_path);
        }
        
        Ok(Self {
            proxy_addr,
            client,
            username,
            password,
        })
    }
    
    /// Load CA certificate from file
    fn load_ca_certificate<P: AsRef<Path>>(path: P) -> NetworkResult<Certificate> {
        let cert_data = std::fs::read(&path)
            .map_err(|e| NetworkError::Transport {
                reason: format!("Failed to read CA certificate file: {}", e)
            })?;
        
        Certificate::from_pem(&cert_data)
            .map_err(|e| NetworkError::Transport {
                reason: format!("Failed to parse CA certificate: {}", e)
            })
    }
    
    /// Get the underlying reqwest client
    pub fn client(&self) -> &Client {
        &self.client
    }
    
    /// Create a GET request through the proxy
    pub fn get(&self, url: &str) -> RequestBuilder {
        debug!("Creating GET request through HTTP proxy: {}", url);
        self.client.get(url)
    }
    
    /// Create a POST request through the proxy
    pub fn post(&self, url: &str) -> RequestBuilder {
        debug!("Creating POST request through HTTP proxy: {}", url);
        self.client.post(url)
    }
    
    /// Create a PUT request through the proxy
    pub fn put(&self, url: &str) -> RequestBuilder {
        debug!("Creating PUT request through HTTP proxy: {}", url);
        self.client.put(url)
    }
    
    /// Create a DELETE request through the proxy
    pub fn delete(&self, url: &str) -> RequestBuilder {
        debug!("Creating DELETE request through HTTP proxy: {}", url);
        self.client.delete(url)
    }
    
    /// Create a HEAD request through the proxy
    pub fn head(&self, url: &str) -> RequestBuilder {
        debug!("Creating HEAD request through HTTP proxy: {}", url);
        self.client.head(url)
    }
    
    /// Create a PATCH request through the proxy
    pub fn patch(&self, url: &str) -> RequestBuilder {
        debug!("Creating PATCH request through HTTP proxy: {}", url);
        self.client.patch(url)
    }
    
    /// Get proxy address
    pub fn proxy_addr(&self) -> SocketAddr {
        self.proxy_addr
    }
    
    /// Check if authentication is configured
    pub fn has_auth(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }
}