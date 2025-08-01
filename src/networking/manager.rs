//! Network Manager - Orchestrates the entire networking system
//! 
//! This module provides the central coordination point for all networking
//! operations, managing multiple circuits, connection state, and event dispatch.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::client::Client;
use crate::networking::core::Core;
use crate::networking::circuit::{Circuit, CircuitOptions, CircuitEvent};
use crate::networking::auth::{SessionInfo, AuthenticationService};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, broadcast};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// Network connection status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkStatus {
    Idle,
    Authenticating,
    Connecting,
    Connected,
    Reconnecting,
    Disconnecting,
    Disconnected,
}

/// Network manager events
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Network status changed
    StatusChanged { old: NetworkStatus, new: NetworkStatus },
    /// Connected to main simulator
    Connected { session: SessionInfo },
    /// Disconnected from network
    Disconnected { reason: String },
    /// Circuit connected
    CircuitConnected { address: SocketAddr },
    /// Circuit disconnected
    CircuitDisconnected { address: SocketAddr, reason: String },
    /// Network error occurred
    Error { error: NetworkError },
}

/// Central network manager coordinating all networking operations
pub struct NetworkManager {
    /// Current network status
    status: Arc<RwLock<NetworkStatus>>,
    
    /// Networking core
    core: Option<Arc<Core>>,
    
    /// Authentication service
    auth_service: AuthenticationService,
    
    /// Active circuits by address
    circuits: Arc<RwLock<HashMap<SocketAddr, Arc<Circuit>>>>,
    
    /// Primary circuit (main simulator connection)
    primary_circuit: Arc<RwLock<Option<Arc<Circuit>>>>,
    
    /// Current session information
    session: Arc<RwLock<Option<SessionInfo>>>,
    
    /// Event broadcaster
    event_tx: broadcast::Sender<NetworkEvent>,
    
    /// Background task handles
    background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl NetworkManager {
    /// Create a new network manager
    pub fn new(capacity: usize) -> Self {
        let (event_tx, _) = broadcast::channel(capacity);
        
        Self {
            status: Arc::new(RwLock::new(NetworkStatus::Idle)),
            core: None,
            auth_service: AuthenticationService::new(),
            circuits: Arc::new(RwLock::new(HashMap::new())),
            primary_circuit: Arc::new(RwLock::new(None)),
            session: Arc::new(RwLock::new(None)),
            event_tx,
            background_tasks: Vec::new(),
        }
    }
    
    /// Get current network status
    pub async fn status(&self) -> NetworkStatus {
        self.status.read().await.clone()
    }
    
    /// Get current session info
    pub async fn session(&self) -> Option<SessionInfo> {
        self.session.read().await.clone()
    }
    
    /// Subscribe to network events
    pub fn subscribe(&self) -> broadcast::Receiver<NetworkEvent> {
        self.event_tx.subscribe()
    }
    
    /// Start the network manager with authentication
    pub async fn connect(&mut self, credentials: crate::networking::auth::LoginCredentials) -> NetworkResult<SessionInfo> {
        self.set_status(NetworkStatus::Authenticating).await;
        
        // Authenticate and get session info
        let client = self.auth_service.login(credentials).await?;
        let session_info = self.auth_service.current_session()
            .ok_or(NetworkError::AuthenticationFailed { 
                reason: "No session after login".to_string() 
            })?
            .clone();
        
        // Store session
        *self.session.write().await = Some(session_info.clone());
        
        // Initialize networking core
        let transport_config = crate::networking::transport::TransportConfig::default();
        let core = Arc::new(Core::new(transport_config).await?);
        core.start().await?;
        self.core = Some(Arc::clone(&core));
        
        self.set_status(NetworkStatus::Connecting).await;
        
        // Connect to primary simulator
        let circuit_options = CircuitOptions {
            circuit_code: session_info.circuit_code,
            address: session_info.simulator_address,
            agent_id: session_info.agent_id,
            session_id: session_info.session_id,
        };
        
        let primary = self.connect_circuit(circuit_options).await?;
        
        // Set as primary circuit
        *self.primary_circuit.write().await = Some(Arc::clone(&primary));
        
        self.set_status(NetworkStatus::Connected).await;
        
        // Emit connected event
        self.emit_event(NetworkEvent::Connected { session: session_info.clone() });
        
        // Start background tasks
        self.start_background_tasks().await;
        
        info!("Network manager connected successfully");
        Ok(session_info)
    }
    
    /// Connect to an additional circuit (for region crossing, etc.)
    pub async fn connect_circuit(&self, options: CircuitOptions) -> NetworkResult<Arc<Circuit>> {
        let core = self.core.as_ref()
            .ok_or(NetworkError::Transport { 
                reason: "Core not initialized".to_string() 
            })?;
        
        debug!("Connecting to circuit at {}", options.address);
        
        // Create circuit
        let address = options.address;
        // Create handshake channel
        let (handshake_tx, _handshake_rx) = tokio::sync::mpsc::channel(100);
        let circuit = core.connect_circuit(options, handshake_tx).await?;
        
        // Store in circuits map
        {
            let mut circuits = self.circuits.write().await;
            circuits.insert(address, Arc::clone(&circuit));
        }
        
        // Emit circuit connected event
        self.emit_event(NetworkEvent::CircuitConnected { 
            address 
        });
        
        Ok(circuit)
    }
    
    /// Disconnect from a specific circuit
    pub async fn disconnect_circuit(&self, address: SocketAddr) -> NetworkResult<()> {
        let circuit = {
            let mut circuits = self.circuits.write().await;
            circuits.remove(&address)
        };
        
        if let Some(circuit) = circuit {
            circuit.stop().await?;
            
            // If this was the primary circuit, clear it
            {
                let mut primary = self.primary_circuit.write().await;
                if let Some(ref p) = *primary {
                    if p.address() == address {
                        *primary = None;
                    }
                }
            }
            
            self.emit_event(NetworkEvent::CircuitDisconnected { 
                address, 
                reason: "Requested disconnect".to_string() 
            });
        }
        
        Ok(())
    }
    
    /// Get the primary circuit
    pub async fn primary_circuit(&self) -> Option<Arc<Circuit>> {
        self.primary_circuit.read().await.clone()
    }
    
    /// Get a specific circuit by address
    pub async fn get_circuit(&self, address: SocketAddr) -> Option<Arc<Circuit>> {
        self.circuits.read().await.get(&address).cloned()
    }
    
    /// Disconnect from all circuits and shutdown
    pub async fn disconnect(&mut self) -> NetworkResult<()> {
        self.set_status(NetworkStatus::Disconnecting).await;
        
        // Cancel background tasks
        for handle in self.background_tasks.drain(..) {
            handle.abort();
        }
        
        // Disconnect all circuits
        let circuits: Vec<_> = {
            let circuits_guard = self.circuits.read().await;
            circuits_guard.keys().copied().collect()
        };
        
        for address in circuits {
            if let Err(e) = self.disconnect_circuit(address).await {
                warn!("Error disconnecting circuit {}: {}", address, e);
            }
        }
        
        // Shutdown core
        if let Some(core) = self.core.take() {
            core.shutdown().await?;
        }
        
        // Clear session
        self.auth_service.logout();
        *self.session.write().await = None;
        
        self.set_status(NetworkStatus::Disconnected).await;
        
        self.emit_event(NetworkEvent::Disconnected { 
            reason: "User requested disconnect".to_string() 
        });
        
        info!("Network manager disconnected");
        Ok(())
    }
    
    /// Set network status and emit event if changed
    async fn set_status(&self, new_status: NetworkStatus) {
        let old_status = {
            let mut status = self.status.write().await;
            let old = status.clone();
            *status = new_status.clone();
            old
        };
        
        if old_status != new_status {
            self.emit_event(NetworkEvent::StatusChanged { 
                old: old_status, 
                new: new_status 
            });
        }
    }
    
    /// Emit a network event
    fn emit_event(&self, event: NetworkEvent) {
        if self.event_tx.send(event).is_err() {
            // No subscribers, which is fine
        }
    }
    
    /// Start background monitoring tasks
    async fn start_background_tasks(&mut self) {
        // Connection health monitor
        let health_task = self.start_health_monitor().await;
        self.background_tasks.push(health_task);
        
        // Circuit event handler
        let events_task = self.start_circuit_event_handler().await;
        self.background_tasks.push(events_task);
    }
    
    /// Start connection health monitoring
    async fn start_health_monitor(&self) -> tokio::task::JoinHandle<()> {
        let circuits = Arc::clone(&self.circuits);
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Check circuit health
                let circuit_addresses: Vec<_> = {
                    circuits.read().await.keys().copied().collect()
                };
                
                for address in circuit_addresses {
                    // In a real implementation, we'd ping circuits and check health
                    // For now, just log that we're monitoring
                    debug!("Health check for circuit {}", address);
                }
            }
        })
    }
    
    /// Start circuit event handling
    async fn start_circuit_event_handler(&self) -> tokio::task::JoinHandle<()> {
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            // In a real implementation, we'd listen for circuit events
            // and forward them through the network manager
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                // Handle circuit events here
            }
        })
    }
}