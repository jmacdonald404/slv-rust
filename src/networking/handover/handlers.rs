//! Packet handlers for region crossing operations
//!
//! This module contains packet handlers that specifically deal with
//! region crossing and simulator handover logic.

use crate::networking::{NetworkResult, NetworkError};
use crate::networking::handlers::{TypedPacketHandler, HandlerContext};
use crate::networking::packets::generated::*;
use super::RegionCrossingManager;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn, error};

/// Handler for EnableSimulator packets that initiate region crossings
pub struct EnableSimulatorHandler {
    crossing_manager: Arc<RegionCrossingManager>,
}

impl EnableSimulatorHandler {
    pub fn new(crossing_manager: Arc<RegionCrossingManager>) -> Self {
        Self { crossing_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<EnableSimulator> for EnableSimulatorHandler {
    async fn handle_typed(&self, enable_sim: EnableSimulator, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Received EnableSimulator for region {:016x} at {}:{}", 
              enable_sim.handle,
              enable_sim.ip.to_std_addr(),
              enable_sim.port.to_host_order());
        
        debug!("🌍 EnableSimulator details:");
        debug!("   Handle: {:016x}", enable_sim.handle);
        
        // Forward to region crossing manager
        if let Err(e) = self.crossing_manager.handle_enable_simulator(enable_sim).await {
            error!("❌ Failed to handle EnableSimulator: {}", e);
            return Err(e);
        }
        
        Ok(())
    }
}

/// Handler for DisableSimulator packets that terminate region connections
pub struct DisableSimulatorHandler {
    crossing_manager: Arc<RegionCrossingManager>,
}

impl DisableSimulatorHandler {
    pub fn new(crossing_manager: Arc<RegionCrossingManager>) -> Self {
        Self { crossing_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<DisableSimulator> for DisableSimulatorHandler {
    async fn handle_typed(&self, disable_sim: DisableSimulator, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Received DisableSimulator");
        
        debug!("🌍 DisableSimulator details: {:?}", disable_sim);
        
        // Note: DisableSimulator packet structure may not include region_handle
        // This is a placeholder until we know the correct field structure
        
        Ok(())
    }
}

/// Handler for TeleportStart packets that indicate the beginning of a teleport
pub struct TeleportStartHandler {
    crossing_manager: Arc<RegionCrossingManager>,
}

impl TeleportStartHandler {
    pub fn new(crossing_manager: Arc<RegionCrossingManager>) -> Self {
        Self { crossing_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<TeleportStart> for TeleportStartHandler {
    async fn handle_typed(&self, teleport_start: TeleportStart, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Received TeleportStart - teleport flags: {}", teleport_start.teleport_flags);
        
        debug!("🌍 TeleportStart details:");
        debug!("   Flags: 0x{:08x}", teleport_start.teleport_flags);
        
        // For teleports, we typically wait for EnableSimulator to follow
        // This handler mainly logs the teleport initiation
        
        Ok(())
    }
}

/// Handler for TeleportProgress packets that provide teleport status updates
pub struct TeleportProgressHandler;

impl TeleportProgressHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<TeleportProgress> for TeleportProgressHandler {
    async fn handle_typed(&self, teleport_progress: TeleportProgress, _context: &HandlerContext) -> NetworkResult<()> {
        debug!("🌍 Teleport progress: teleport_flags={} - message={:?}", 
               teleport_progress.teleport_flags,
               teleport_progress.message);
        
        Ok(())
    }
}

/// Handler for TeleportFinish packets that complete teleports
pub struct TeleportFinishHandler {
    crossing_manager: Arc<RegionCrossingManager>,
}

impl TeleportFinishHandler {
    pub fn new(crossing_manager: Arc<RegionCrossingManager>) -> Self {
        Self { crossing_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<TeleportFinish> for TeleportFinishHandler {
    async fn handle_typed(&self, teleport_finish: TeleportFinish, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Received TeleportFinish");
        
        debug!("🌍 TeleportFinish details:");
        debug!("   Agent ID: {}", teleport_finish.agent_id);
        debug!("   Location ID: {}", teleport_finish.location_id);
        debug!("   Sim IP: {:?}", teleport_finish.sim_ip);
        debug!("   Sim Port: {:?}", teleport_finish.sim_port);
        
        // The teleport is complete - we should be connected to the new region
        // by now through the EnableSimulator -> UseCircuitCode flow
        
        Ok(())
    }
}

/// Handler for TeleportFailed packets that indicate teleport failures
pub struct TeleportFailedHandler;

impl TeleportFailedHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<TeleportFailed> for TeleportFailedHandler {
    async fn handle_typed(&self, teleport_failed: TeleportFailed, _context: &HandlerContext) -> NetworkResult<()> {
        warn!("🌍 Teleport failed: {:?} (reason: {:?})", 
              teleport_failed.alert_info,
              teleport_failed.reason);
        
        Ok(())
    }
}

/// Handler for TeleportCancel packets that cancel ongoing teleports
pub struct TeleportCancelHandler;

impl TeleportCancelHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<TeleportCancel> for TeleportCancelHandler {
    async fn handle_typed(&self, _teleport_cancel: TeleportCancel, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Teleport cancelled");
        
        Ok(())
    }
}

/// Handler for CrossedRegion packets that indicate successful region crossings
pub struct CrossedRegionHandler {
    crossing_manager: Arc<RegionCrossingManager>,
}

impl CrossedRegionHandler {
    pub fn new(crossing_manager: Arc<RegionCrossingManager>) -> Self {
        Self { crossing_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<CrossedRegion> for CrossedRegionHandler {
    async fn handle_typed(&self, crossed_region: CrossedRegion, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🌍 Crossed into region {:016x}", crossed_region.region_handle);
        
        debug!("🌍 CrossedRegion details:");
        debug!("   Session ID: {}", crossed_region.session_id);
        debug!("   Simulator IP: {:?}", crossed_region.sim_ip);
        debug!("   Simulator Port: {:?}", crossed_region.sim_port);
        debug!("   Seed Capability: {:?}", crossed_region.seed_capability);
        
        // The region crossing is complete
        // We may need to update our capabilities with the new seed capability
        
        Ok(())
    }
}

/// Handler for EstablishAgentCommunication packets  
pub struct EstablishAgentCommunicationHandler;

impl EstablishAgentCommunicationHandler {
    pub fn new() -> Self {
        Self
    }
}

// Note: EstablishAgentCommunication packet may not be defined in the current message template
// This is a placeholder implementation that would be activated when the packet is available

/// Handler for ConfirmEnableSimulator packets that confirm simulator enablement
pub struct ConfirmEnableSimulatorHandler;

impl ConfirmEnableSimulatorHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<ConfirmEnableSimulator> for ConfirmEnableSimulatorHandler {
    async fn handle_typed(&self, _confirm_enable: ConfirmEnableSimulator, _context: &HandlerContext) -> NetworkResult<()> {
        debug!("🌍 Confirming EnableSimulator");
        
        // This is typically sent by the client to confirm that it has processed
        // an EnableSimulator packet and is ready to connect to the new simulator
        
        Ok(())
    }
}