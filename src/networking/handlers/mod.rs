//! Packet handlers for processing Second Life protocol messages
//! 
//! This module provides an async, event-driven packet handling system
//! that processes incoming packets and triggers appropriate responses.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::{Packet, PacketWrapper};
use crate::networking::circuit::Circuit;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn, info};

pub mod login_handlers;
pub mod agent_handlers;
pub mod region_handlers;
pub mod inventory_handlers;
pub mod asset_handlers;
pub mod system;

// Re-export handlers for easy access
pub use region_handlers::*;
pub use inventory_handlers::*;
pub use asset_handlers::*;

/// Context provided to packet handlers
#[derive(Debug)]
pub struct HandlerContext {
    /// The circuit that received the packet
    pub circuit: Arc<Circuit>,
    /// Client agent ID
    pub agent_id: uuid::Uuid,
    /// Session ID
    pub session_id: uuid::Uuid,
    /// Handshake event sender
    pub handshake_tx: tokio::sync::mpsc::Sender<crate::networking::client::HandshakeEvent>,
}

/// Async packet handler trait
#[async_trait]
pub trait PacketHandler: Send + Sync {
    /// Handle a packet
    async fn handle(&self, packet: PacketWrapper, context: &HandlerContext) -> NetworkResult<()>;
    
    /// Get the packet type this handler processes
    fn packet_type(&self) -> u32; // Packet lookup key
    
    /// Get handler name for debugging
    fn name(&self) -> &'static str;
}

/// Typed packet handler for specific packet types
#[async_trait]
pub trait TypedPacketHandler<P: Packet>: Send + Sync {
    /// Handle a typed packet
    async fn handle_typed(&self, packet: P, context: &HandlerContext) -> NetworkResult<()>;
}

/// Wrapper to make typed handlers work with the generic handler system
pub struct TypedHandlerWrapper<P: Packet, H: TypedPacketHandler<P>> {
    handler: H,
    _phantom: std::marker::PhantomData<P>,
}

impl<P: Packet, H: TypedPacketHandler<P>> TypedHandlerWrapper<P, H> {
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<P: Packet + 'static, H: TypedPacketHandler<P>> PacketHandler for TypedHandlerWrapper<P, H> {
    async fn handle(&self, packet: PacketWrapper, context: &HandlerContext) -> NetworkResult<()> {
        // Deserialize the packet
        let typed_packet: P = packet.deserialize()?;
        
        // Handle the typed packet
        self.handler.handle_typed(typed_packet, context).await
    }
    
    fn packet_type(&self) -> u32 {
        P::lookup_key()
    }
    
    fn name(&self) -> &'static str {
        P::name()
    }
}

/// Packet handler registry
pub struct PacketHandlerRegistry {
    handlers: RwLock<HashMap<u32, Arc<dyn PacketHandler>>>,
}

impl PacketHandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register a packet handler
    pub async fn register<H: PacketHandler + 'static>(&self, handler: H) {
        let mut handlers = self.handlers.write().await;
        let packet_type = handler.packet_type();
        let name = handler.name();
        
        info!("Registering packet handler for {}: {}", packet_type, name);
        handlers.insert(packet_type, Arc::new(handler));
    }
    
    /// Register a typed packet handler
    pub async fn register_typed<P, H>(&self, handler: H)
    where
        P: Packet + 'static,
        H: TypedPacketHandler<P> + 'static,
    {
        let wrapper = TypedHandlerWrapper::new(handler);
        self.register(wrapper).await;
    }
    
    /// Handle a packet by dispatching to the appropriate handler
    pub async fn handle_packet(&self, packet: PacketWrapper, context: &HandlerContext) -> NetworkResult<()> {
        let packet_key = match packet.frequency {
            crate::networking::packets::PacketFrequency::High => packet.packet_id as u32,
            crate::networking::packets::PacketFrequency::Medium => (1 << 16) | (packet.packet_id as u32),
            crate::networking::packets::PacketFrequency::Low => (2 << 16) | (packet.packet_id as u32),
            crate::networking::packets::PacketFrequency::Fixed => (3 << 16) | (packet.packet_id as u32),
        };
        
        info!("🔄 PACKET HANDLER: Processing packet from {}", context.circuit.address());
        info!("   Packet ID: {}", packet.packet_id);
        info!("   Frequency: {:?}", packet.frequency);
        info!("   Handler Key: {}", packet_key);
        info!("   Reliable: {}", packet.reliable);
        
        let handlers = self.handlers.read().await;
        
        if let Some(handler) = handlers.get(&packet_key) {
            info!("🎯 PACKET HANDLER: Dispatching to handler '{}'", handler.name());
            
            let start_time = std::time::Instant::now();
            let result = handler.handle(packet, context).await;
            let processing_time = start_time.elapsed();
            
            match &result {
                Ok(()) => {
                    info!("✅ PACKET HANDLER RESPONSE: Successfully handled packet with '{}'", handler.name());
                    info!("   Processing time: {:?}", processing_time);
                }
                Err(e) => {
                    warn!("❌ PACKET HANDLER RESPONSE ERROR: Handler '{}' failed", handler.name());
                    warn!("   Error: {}", e);
                    warn!("   Processing time: {:?}", processing_time);
                }
            }
            
            result
        } else {
            warn!("⚠️ PACKET HANDLER: No handler registered for packet type {}", packet_key);
            warn!("   Packet ID: {}", packet.packet_id);
            warn!("   Frequency: {:?}", packet.frequency);
            warn!("   This packet will be ignored");
            // Don't return an error for unhandled packets - just ignore them
            Ok(())
        }
    }
    
    /// Get count of registered handlers
    pub async fn handler_count(&self) -> usize {
        self.handlers.read().await.len()
    }
    
    /// Initialize default handlers
    pub async fn init_default_handlers(&self) {
        // Register the comprehensive RegionHandshakeHandler from login_handlers
        self.register_typed(login_handlers::RegionHandshakeHandler::new()).await;
        self.register_typed(agent_handlers::PacketAckHandler::new()).await;
        
        // Register the critical handlers for auth handshake
        self.register_typed(login_handlers::StartPingCheckHandler::new()).await;
        self.register_typed(login_handlers::CompletePingCheckHandler::new()).await;
        self.register_typed(login_handlers::ObjectUpdateHandler::new()).await;
        self.register_typed(login_handlers::LayerDataHandler::new()).await;
        self.register_typed(login_handlers::CoarseLocationUpdateHandler::new()).await;
        
        // Register additional essential handlers
        self.register_typed(login_handlers::EconomyDataHandler::new()).await;
        self.register_typed(login_handlers::UUIDNameReplyHandler::new()).await;
        self.register_typed(login_handlers::EnableSimulatorHandler::new()).await;
        
        // Register the CRITICAL handler for completing handshake
        self.register_typed(login_handlers::AgentMovementCompleteHandler::new()).await;
        
        // Register comprehensive object/prim management handlers per netplan.md
        self.register_typed(region_handlers::ObjectUpdateHandler::new()).await;
        self.register_typed(region_handlers::ObjectUpdateCompressedHandler::new()).await;
        self.register_typed(region_handlers::ObjectUpdateCachedHandler::new()).await;
        self.register_typed(region_handlers::KillObjectHandler::new()).await;
        self.register_typed(region_handlers::ImprovedTerseObjectUpdateHandler::new()).await;
        
        // Register inventory system handlers per netplan.md
        self.register_typed(inventory_handlers::FetchInventoryDescendentsHandler::new()).await;
        self.register_typed(inventory_handlers::InventoryDescendentsHandler::new()).await;
        self.register_typed(inventory_handlers::UpdateInventoryItemHandler::new()).await;
        
        // Register asset transfer system handlers per netplan.md
        // Note: These handlers need an AssetManager instance to be fully functional
        // For now, we'll create a default asset manager for compatibility
        let asset_manager = Arc::new(crate::networking::assets::AssetManager::default());
        
        self.register_typed(asset_handlers::TransferRequestHandler::new(Arc::clone(&asset_manager))).await;
        self.register_typed(asset_handlers::TransferInfoHandler::new()).await;
        self.register_typed(asset_handlers::TransferAbortHandler::new()).await;
        self.register_typed(asset_handlers::RequestImageHandler::new(Arc::clone(&asset_manager))).await;
        self.register_typed(asset_handlers::ImageDataHandler::new()).await;
        self.register_typed(asset_handlers::LayerDataHandler::new()).await;
        
        // Register region crossing handlers per netplan.md
        // Note: These handlers need a RegionCrossingManager instance to be fully functional
        // For now, we'll create a default manager for compatibility
        let crossing_manager = Arc::new(crate::networking::handover::RegionCrossingManager::new(
            uuid::Uuid::new_v4(), // placeholder agent_id
            uuid::Uuid::new_v4(), // placeholder session_id
        ));
        
        self.register_typed(crate::networking::handover::handlers::EnableSimulatorHandler::new(Arc::clone(&crossing_manager))).await;
        self.register_typed(crate::networking::handover::handlers::DisableSimulatorHandler::new(Arc::clone(&crossing_manager))).await;
        self.register_typed(crate::networking::handover::handlers::TeleportStartHandler::new(Arc::clone(&crossing_manager))).await;
        self.register_typed(crate::networking::handover::handlers::TeleportProgressHandler::new()).await;
        self.register_typed(crate::networking::handover::handlers::TeleportFinishHandler::new(Arc::clone(&crossing_manager))).await;
        self.register_typed(crate::networking::handover::handlers::TeleportFailedHandler::new()).await;
        self.register_typed(crate::networking::handover::handlers::TeleportCancelHandler::new()).await;
        self.register_typed(crate::networking::handover::handlers::CrossedRegionHandler::new(Arc::clone(&crossing_manager))).await;
        // Note: EstablishAgentCommunication handler disabled until packet is available
        // self.register_typed(crate::networking::handover::handlers::EstablishAgentCommunicationHandler::new()).await;
        self.register_typed(crate::networking::handover::handlers::ConfirmEnableSimulatorHandler::new()).await;
        
        // Register temporary raw handlers for missing packets to reduce log spam
        self.register(AttachedSoundRawHandler::new()).await;
        
        info!("Initialized {} default packet handlers", self.handler_count().await);
    }
}

impl Default for PacketHandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Packet processing engine
pub struct PacketProcessor {
    registry: Arc<PacketHandlerRegistry>,
}

impl PacketProcessor {
    pub fn new(registry: Arc<PacketHandlerRegistry>) -> Self {
        Self { registry }
    }
    
    /// Process a packet asynchronously
    pub async fn process_packet(&self, packet: PacketWrapper, context: HandlerContext) -> NetworkResult<()> {
        info!("🏭 PACKET PROCESSOR: Starting packet processing");
        info!("   Source: {}", context.circuit.address());
        info!("   Packet ID: {}", packet.packet_id);
        info!("   Frequency: {:?}", packet.frequency);
        
        let start_time = std::time::Instant::now();
        let result = self.registry.handle_packet(packet, &context).await;
        let total_time = start_time.elapsed();
        
        match &result {
            Ok(()) => {
                info!("✅ PACKET PROCESSOR RESPONSE: Packet processing completed successfully");
                info!("   Total processing time: {:?}", total_time);
            }
            Err(e) => {
                warn!("❌ PACKET PROCESSOR RESPONSE ERROR: Packet processing failed");
                warn!("   Error: {}", e);
                warn!("   Total processing time: {:?}", total_time);
            }
        }
        
        result
    }
    
    /// Start packet processing loop
    pub async fn start_processing(
        &self,
        mut packet_rx: tokio::sync::mpsc::UnboundedReceiver<(PacketWrapper, HandlerContext)>,
    ) {
        info!("🏭 PACKET PROCESSOR: Starting packet processing loop");
        
        let mut packet_count = 0;
        let start_time = std::time::Instant::now();
        
        while let Some((packet, context)) = packet_rx.recv().await {
            packet_count += 1;
            
            info!("🏭 PACKET PROCESSOR: Received packet #{} from queue", packet_count);
            info!("   Packet ID: {}", packet.packet_id);
            info!("   Frequency: {:?}", packet.frequency);
            info!("   Source: {}", context.circuit.address());
            
            if let Err(e) = self.process_packet(packet, context).await {
                warn!("❌ PACKET PROCESSOR: Error processing packet #{}: {}", packet_count, e);
            }
            
            // Log statistics every 100 packets
            if packet_count % 100 == 0 {
                let elapsed = start_time.elapsed();
                let packets_per_sec = packet_count as f64 / elapsed.as_secs_f64();
                info!("📊 PACKET PROCESSOR STATS: Processed {} packets in {:?} ({:.2} packets/sec)", 
                      packet_count, elapsed, packets_per_sec);
            }
        }
        
        let total_time = start_time.elapsed();
        info!("🏭 PACKET PROCESSOR: Stopped after processing {} packets in {:?}", packet_count, total_time);
    }
}

/// Temporary raw handler for AttachedSound packets (Medium ID 13, type 65549)
/// Reduces log spam until proper AttachedSound struct is generated
pub struct AttachedSoundRawHandler;

impl AttachedSoundRawHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PacketHandler for AttachedSoundRawHandler {
    async fn handle(&self, packet: PacketWrapper, _context: &HandlerContext) -> NetworkResult<()> {
        debug!("🔊 AttachedSound packet received ({} bytes) - audio system not implemented", 
               packet.data.len());
        
        // Basic validation - AttachedSound should have at least:
        // SoundID (16) + ObjectID (16) + OwnerID (16) + Gain (4) + Flags (1) = 53 bytes minimum
        if packet.data.len() < 53 {
            warn!("🔊 AttachedSound packet too short: {} bytes (expected >= 53)", packet.data.len());
        }
        
        // TODO: Parse sound data when AttachedSound struct is generated
        // TODO: Implement audio system integration
        Ok(())
    }
    
    fn packet_type(&self) -> u32 {
        65549 // Medium frequency (1 << 16) + packet ID 13
    }
    
    fn name(&self) -> &'static str {
        "AttachedSoundRawHandler"
    }
}