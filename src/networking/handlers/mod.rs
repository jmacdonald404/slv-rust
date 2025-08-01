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
pub mod system;

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
        
        info!("Processing packet: id={}, frequency={:?}, key={}", packet.packet_id, packet.frequency, packet_key);
        
        let handlers = self.handlers.read().await;
        
        if let Some(handler) = handlers.get(&packet_key) {
            info!("Handling packet type {} with {}", packet_key, handler.name());
            handler.handle(packet, context).await
        } else {
            warn!("No handler registered for packet type {} (id={}, frequency={:?})", packet_key, packet.packet_id, packet.frequency);
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
        self.registry.handle_packet(packet, &context).await
    }
    
    /// Start packet processing loop
    pub async fn start_processing(
        &self,
        mut packet_rx: tokio::sync::mpsc::UnboundedReceiver<(PacketWrapper, HandlerContext)>,
    ) {
        info!("Starting packet processor");
        
        while let Some((packet, context)) = packet_rx.recv().await {
            info!("Packet processor received packet: id={}, frequency={:?}", packet.packet_id, packet.frequency);
            if let Err(e) = self.process_packet(packet, context).await {
                warn!("Error processing packet: {}", e);
            }
        }
        
        info!("Packet processor stopped");
    }
}