//! Simulator handover logic for region crossings
//!
//! This module implements the state machine and logic for moving between regions
//! in Second Life, handling the complex process of connecting to new simulators
//! while gracefully disconnecting from old ones per netplan.md.

use crate::networking::{NetworkResult, NetworkError};
use crate::networking::circuit::{Circuit, CircuitOptions};
use crate::networking::packets::generated::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

pub mod state_machine;
pub mod handlers;

pub use state_machine::{RegionCrossingStateMachine, RegionCrossingState, RegionCrossingEvent};

/// Information about a simulator for region crossing
#[derive(Debug, Clone)]
pub struct SimulatorInfo {
    /// Simulator IP address and port
    pub address: SocketAddr,
    /// Region handle for this simulator
    pub region_handle: u64,
    /// Region UUID
    pub region_id: Uuid,
    /// Circuit code for establishing connection
    pub circuit_code: u32,
    /// Seed capability URL for this region
    pub seed_capability: Option<String>,
}

/// Region crossing manager that handles the complex state machine
/// for moving between simulators in Second Life
#[derive(Debug)]
pub struct RegionCrossingManager {
    /// Active circuits to simulators by region handle
    circuits: Arc<RwLock<HashMap<u64, Arc<Circuit>>>>,
    /// Current primary region handle
    current_region: Arc<RwLock<Option<u64>>>,
    /// Region crossing state machine
    state_machine: Arc<RwLock<RegionCrossingStateMachine>>,
    /// Channel for region crossing events
    event_tx: mpsc::UnboundedSender<RegionCrossingEvent>,
    event_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<RegionCrossingEvent>>>>,
    /// Agent session information
    agent_id: Uuid,
    session_id: Uuid,
}

impl RegionCrossingManager {
    /// Create a new region crossing manager
    pub fn new(agent_id: Uuid, session_id: Uuid) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            circuits: Arc::new(RwLock::new(HashMap::new())),
            current_region: Arc::new(RwLock::new(None)),
            state_machine: Arc::new(RwLock::new(RegionCrossingStateMachine::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
            agent_id,
            session_id,
        }
    }
    
    /// Start the region crossing manager event loop
    pub fn start(self: Arc<Self>) -> NetworkResult<()> {
        let manager_clone = Arc::clone(&self);
        
        tokio::spawn(async move {
            let event_rx = {
                let mut rx_lock = manager_clone.event_rx.write().await;
                match rx_lock.take() {
                    Some(rx) => rx,
                    None => {
                        error!("❌ Region crossing manager already started");
                        return;
                    }
                }
            };
            
            manager_clone.event_loop(event_rx).await;
        });
        
        info!("🌍 Region crossing manager started");
        Ok(())
    }
    
    /// Add a circuit for a specific region
    pub async fn add_circuit(&self, region_handle: u64, circuit: Arc<Circuit>) {
        let mut circuits = self.circuits.write().await;
        circuits.insert(region_handle, circuit);
        
        // If this is our first circuit, make it the current region
        let mut current = self.current_region.write().await;
        if current.is_none() {
            *current = Some(region_handle);
            info!("🌍 Set initial region: {:016x}", region_handle);
        }
        
        debug!("🌍 Added circuit for region {:016x}", region_handle);
    }
    
    /// Get the current primary region handle
    pub async fn current_region(&self) -> Option<u64> {
        *self.current_region.read().await
    }
    
    /// Get a circuit for a specific region
    pub async fn get_circuit(&self, region_handle: u64) -> Option<Arc<Circuit>> {
        let circuits = self.circuits.read().await;
        circuits.get(&region_handle).cloned()
    }
    
    /// Initiate a region crossing to a new simulator
    pub async fn initiate_crossing(&self, simulator_info: SimulatorInfo) -> NetworkResult<()> {
        info!("🌍 Initiating region crossing to {:016x} at {}", 
              simulator_info.region_handle, simulator_info.address);
        
        // Send event to state machine
        self.event_tx.send(RegionCrossingEvent::InitiateCrossing {
            simulator_info
        }).map_err(|e| NetworkError::Other {
            reason: format!("Failed to send crossing event: {}", e)
        })?;
        
        Ok(())
    }
    
    /// Handle EnableSimulator packet for region crossing
    pub async fn handle_enable_simulator(&self, enable_sim: EnableSimulator) -> NetworkResult<()> {
        debug!("🌍 Received EnableSimulator for region {:016x}", enable_sim.handle);
        
        // Extract simulator information
        let simulator_info = SimulatorInfo {
            address: SocketAddr::new(
                std::net::IpAddr::V4(enable_sim.ip.to_std_addr()),
                enable_sim.port.to_host_order()
            ),
            region_handle: enable_sim.handle,
            region_id: uuid::Uuid::new_v4(), // EnableSimulator doesn't provide region_id
            circuit_code: 0, // EnableSimulator doesn't provide circuit_code
            seed_capability: None, // Will be provided later
        };
        
        // Send event to state machine
        self.event_tx.send(RegionCrossingEvent::EnableSimulatorReceived {
            simulator_info
        }).map_err(|e| NetworkError::Other {
            reason: format!("Failed to send enable simulator event: {}", e)
        })?;
        
        Ok(())
    }
    
    /// Complete agent movement to new region
    pub async fn complete_agent_movement(&self, region_handle: u64) -> NetworkResult<()> {
        info!("🌍 Completing agent movement to region {:016x}", region_handle);
        
        let circuit = self.get_circuit(region_handle).await.ok_or_else(|| {
            NetworkError::CircuitNotFound { id: region_handle as u32 }
        })?;
        
        // Send CompleteAgentMovement packet
        let complete_movement = CompleteAgentMovement {
            agent_id: self.agent_id,
            session_id: self.session_id,
            circuit_code: 0, // Will be filled by circuit
        };
        
        circuit.send_reliable(&complete_movement, std::time::Duration::from_secs(5)).await?;
        
        // Update current region
        {
            let mut current = self.current_region.write().await;
            *current = Some(region_handle);
        }
        
        info!("✅ Agent movement completed to region {:016x}", region_handle);
        Ok(())
    }
    
    /// Gracefully disconnect from a region
    pub async fn disconnect_from_region(&self, region_handle: u64) -> NetworkResult<()> {
        info!("🌍 Disconnecting from region {:016x}", region_handle);
        
        let circuit = {
            let mut circuits = self.circuits.write().await;
            circuits.remove(&region_handle)
        };
        
        if let Some(circuit) = circuit {
            // Send CloseCircuit packet to gracefully disconnect
            let close_circuit = CloseCircuit {};
            
            if let Err(e) = circuit.send_reliable(&close_circuit, std::time::Duration::from_secs(5)).await {
                warn!("⚠️ Failed to send CloseCircuit to {:016x}: {}", region_handle, e);
            }
            
            // Allow some time for the packet to be sent
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        
        info!("✅ Disconnected from region {:016x}", region_handle);
        Ok(())
    }
    
    /// Get all active region handles
    pub async fn active_regions(&self) -> Vec<u64> {
        let circuits = self.circuits.read().await;
        circuits.keys().copied().collect()
    }
    
    /// Event processing loop for the region crossing state machine
    async fn event_loop(&self, mut event_rx: mpsc::UnboundedReceiver<RegionCrossingEvent>) {
        info!("🌍 Region crossing event loop started");
        
        while let Some(event) = event_rx.recv().await {
            debug!("🌍 Processing region crossing event: {:?}", event);
            
            let result = match event {
                RegionCrossingEvent::InitiateCrossing { simulator_info } => {
                    self.process_initiate_crossing(simulator_info).await
                },
                RegionCrossingEvent::EnableSimulatorReceived { simulator_info } => {
                    self.process_enable_simulator(simulator_info).await
                },
                RegionCrossingEvent::ConnectionEstablished { region_handle } => {
                    self.process_connection_established(region_handle).await
                },
                RegionCrossingEvent::MovementCompleted { region_handle } => {
                    self.process_movement_completed(region_handle).await
                },
                RegionCrossingEvent::CrossingFailed { region_handle, error } => {
                    self.process_crossing_failed(region_handle, error).await
                },
            };
            
            if let Err(e) = result {
                error!("❌ Error processing region crossing event: {}", e);
            }
        }
        
        warn!("🌍 Region crossing event loop ended");
    }
    
    /// Process initiate crossing event
    async fn process_initiate_crossing(&self, simulator_info: SimulatorInfo) -> NetworkResult<()> {
        // Update state machine
        {
            let mut state_machine = self.state_machine.write().await;
            state_machine.transition_to(RegionCrossingState::Connecting)?;
        }
        
        // Create new circuit for the destination region
        // Note: This is a placeholder implementation
        // In a real implementation, we would need to create a proper circuit
        // with the appropriate transport (QUIC or UDP)
        warn!("🌍 Circuit creation for region crossing not yet fully implemented");
        Err(NetworkError::Other {
            reason: "Circuit creation for region crossing not yet implemented".to_string()
        })
    }
    
    /// Process enable simulator event
    async fn process_enable_simulator(&self, simulator_info: SimulatorInfo) -> NetworkResult<()> {
        info!("🌍 Processing EnableSimulator for new region");
        
        // This is typically the first step in a region crossing
        // We'll initiate the crossing process
        self.initiate_crossing(simulator_info).await
    }
    
    /// Process connection established event
    async fn process_connection_established(&self, region_handle: u64) -> NetworkResult<()> {
        info!("🌍 Connection established to region {:016x}", region_handle);
        
        // Update state machine
        {
            let mut state_machine = self.state_machine.write().await;
            state_machine.transition_to(RegionCrossingState::MovingAgent)?;
        }
        
        // Complete agent movement
        self.complete_agent_movement(region_handle).await?;
        
        // Notify movement completed
        self.event_tx.send(RegionCrossingEvent::MovementCompleted {
            region_handle
        }).ok();
        
        Ok(())
    }
    
    /// Process movement completed event
    async fn process_movement_completed(&self, region_handle: u64) -> NetworkResult<()> {
        info!("🌍 Movement completed to region {:016x}", region_handle);
        
        // Update state machine
        {
            let mut state_machine = self.state_machine.write().await;
            state_machine.transition_to(RegionCrossingState::Connected)?;
        }
        
        // Disconnect from old regions (keep only the new one)
        let old_regions: Vec<u64> = {
            let circuits = self.circuits.read().await;
            circuits.keys()
                .filter(|&&handle| handle != region_handle)
                .copied()
                .collect()
        };
        
        for old_region in old_regions {
            if let Err(e) = self.disconnect_from_region(old_region).await {
                warn!("⚠️ Failed to disconnect from old region {:016x}: {}", old_region, e);
            }
        }
        
        info!("✅ Region crossing completed successfully");
        Ok(())
    }
    
    /// Process crossing failed event
    async fn process_crossing_failed(&self, region_handle: u64, error: String) -> NetworkResult<()> {
        error!("❌ Region crossing failed for {:016x}: {}", region_handle, error);
        
        // Update state machine
        {
            let mut state_machine = self.state_machine.write().await;
            state_machine.transition_to(RegionCrossingState::Failed)?;
        }
        
        // Clean up failed connection
        self.disconnect_from_region(region_handle).await.ok();
        
        Ok(())
    }
}