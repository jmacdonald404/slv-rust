//! Packet Handler System - Event-driven packet processing
//! 
//! This module provides a flexible, prioritized packet handling system
//! inspired by homunculus's delegate pattern but adapted for Rust's
//! type system and async/await patterns.

use crate::networking::{NetworkError, NetworkResult};
use crate::networking::packets::{Packet, PacketWrapper};
use crate::networking::circuit::Circuit;
use crate::networking::core::Core;
use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn, error};

/// Context passed to packet handlers
#[derive(Clone)]
pub struct HandlerContext {
    /// The circuit that received the packet
    pub circuit: Arc<Circuit>,
    /// The networking core
    pub core: Arc<Core>,
    /// Additional context data (extensible)
    pub data: Arc<dyn Any + Send + Sync>,
}

impl HandlerContext {
    pub fn new(circuit: Arc<Circuit>, core: Arc<Core>) -> Self {
        Self {
            circuit,
            core,
            data: Arc::new(()),
        }
    }
    
    /// Get typed context data
    pub fn get_data<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.data.downcast_ref::<T>()
    }
    
    /// Create context with additional data
    pub fn with_data<T: Any + Send + Sync>(mut self, data: T) -> Self {
        self.data = Arc::new(data);
        self
    }
}

/// Trait for packet handlers
#[async_trait]
pub trait PacketHandler: Send + Sync + Debug {
    /// Handle a packet
    async fn handle(&self, packet: &PacketWrapper, context: &HandlerContext) -> NetworkResult<()>;
    
    /// Priority level (higher = processed first)
    fn priority(&self) -> i32 {
        0
    }
    
    /// Check if this handler should process the packet
    fn should_handle(&self, packet: &PacketWrapper, context: &HandlerContext) -> bool {
        true
    }
    
    /// Get handler name for debugging
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Typed packet handler for specific packet types
#[async_trait]
pub trait TypedPacketHandler<P>: Send + Sync + Debug
where
    P: Packet + Clone,
{
    /// Handle a typed packet
    async fn handle_typed(&self, packet: &P, context: &HandlerContext) -> NetworkResult<()>;
    
    /// Priority level (higher = processed first)
    fn priority(&self) -> i32 {
        0
    }
    
    /// Check if this handler should process the packet
    fn should_handle_typed(&self, packet: &P, context: &HandlerContext) -> bool {
        true
    }
}

/// Wrapper to adapt TypedPacketHandler to PacketHandler
#[derive(Debug)]
struct TypedHandlerWrapper<P, H>
where
    P: Packet + Clone,
    H: TypedPacketHandler<P>,
{
    handler: H,
    _phantom: std::marker::PhantomData<P>,
}

impl<P, H> TypedHandlerWrapper<P, H>
where
    P: Packet + Clone,
    H: TypedPacketHandler<P>,
{
    fn new(handler: H) -> Self {
        Self {
            handler,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<P, H> PacketHandler for TypedHandlerWrapper<P, H>
where
    P: Packet + Clone + 'static,
    H: TypedPacketHandler<P>,
{
    async fn handle(&self, packet: &PacketWrapper, context: &HandlerContext) -> NetworkResult<()> {
        // Try to deserialize the packet to the expected type
        // In a real implementation, this would use proper packet deserialization
        // For now, we'll just check the packet ID
        if packet.packet_id == P::ID {
            // This is a simplified approach - in reality we'd deserialize the packet data
            // let typed_packet = deserialize_packet::<P>(&packet.data)?;
            // For now, create a default instance (this is not ideal but shows the pattern)
            // self.handler.handle_typed(&typed_packet, context).await
            debug!("Would handle typed packet {} with handler {}", 
                   P::name(), self.handler.name());
            Ok(())
        } else {
            Ok(()) // Not our packet type
        }
    }
    
    fn priority(&self) -> i32 {
        self.handler.priority()
    }
    
    fn should_handle(&self, packet: &PacketWrapper, context: &HandlerContext) -> bool {
        packet.packet_id == P::ID
    }
    
    fn name(&self) -> &'static str {
        self.handler.name()
    }
}

/// Handler configuration
#[derive(Debug)]
struct HandlerConfig {
    handler: Box<dyn PacketHandler>,
    priority: i32,
}

/// Packet handler registry managing all packet handlers
#[derive(Debug)]
pub struct HandlerRegistry {
    /// Handlers by packet ID
    handlers: RwLock<HashMap<u16, Vec<HandlerConfig>>>,
    /// Global handlers (process all packets)
    global_handlers: RwLock<Vec<HandlerConfig>>,
}

impl HandlerRegistry {
    /// Create a new handler registry
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            global_handlers: RwLock::new(Vec::new()),
        }
    }
    
    /// Register a packet handler for a specific packet type
    pub async fn register_handler<P, H>(&self, handler: H)
    where
        P: Packet + Clone + 'static,
        H: TypedPacketHandler<P> + 'static,
    {
        let wrapper = TypedHandlerWrapper::new(handler);
        let priority = wrapper.priority();
        let packet_id = P::ID;
        
        let config = HandlerConfig {
            handler: Box::new(wrapper),
            priority,
        };
        
        let mut handlers = self.handlers.write().await;
        let packet_handlers = handlers.entry(packet_id).or_insert_with(Vec::new);
        packet_handlers.push(config);
        
        // Sort by priority (highest first)
        packet_handlers.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        debug!("Registered handler for packet {} (ID: {})", P::name(), packet_id);
    }
    
    /// Register a raw packet handler
    pub async fn register_raw_handler<H>(&self, packet_id: u16, handler: H)
    where
        H: PacketHandler + 'static,
    {
        let priority = handler.priority();
        let config = HandlerConfig {
            handler: Box::new(handler),
            priority,
        };
        
        let mut handlers = self.handlers.write().await;
        let packet_handlers = handlers.entry(packet_id).or_insert_with(Vec::new);
        packet_handlers.push(config);
        
        // Sort by priority (highest first)
        packet_handlers.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        debug!("Registered raw handler for packet ID {}", packet_id);
    }
    
    /// Register a global handler that processes all packets
    pub async fn register_global_handler<H>(&self, handler: H)
    where
        H: PacketHandler + 'static,
    {
        let priority = handler.priority();
        let config = HandlerConfig {
            handler: Box::new(handler),
            priority,
        };
        
        let mut global_handlers = self.global_handlers.write().await;
        global_handlers.push(config);
        
        // Sort by priority (highest first)
        global_handlers.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        debug!("Registered global handler: {}", config.handler.name());
    }
    
    /// Process a packet through all relevant handlers
    pub async fn process_packet(&self, packet: &PacketWrapper, context: &HandlerContext) -> NetworkResult<()> {
        let packet_id = packet.packet_id;
        
        // Get handlers for this packet type
        let handlers = {
            let handlers_guard = self.handlers.read().await;
            handlers_guard.get(&packet_id).cloned().unwrap_or_default()
        };
        
        // Get global handlers
        let global_handlers = {
            let global_guard = self.global_handlers.read().await;
            global_guard.clone()
        };
        
        // Process global handlers first
        for config in &global_handlers {
            if config.handler.should_handle(packet, context) {
                if let Err(e) = config.handler.handle(packet, context).await {
                    error!("Global handler {} failed: {}", config.handler.name(), e);
                    // Continue processing other handlers
                }
            }
        }
        
        // Process specific handlers
        for config in &handlers {
            if config.handler.should_handle(packet, context) {
                if let Err(e) = config.handler.handle(packet, context).await {
                    error!("Handler {} failed for packet ID {}: {}", 
                           config.handler.name(), packet_id, e);
                    // Continue processing other handlers
                }
            }
        }
        
        if handlers.is_empty() && global_handlers.is_empty() {
            debug!("No handlers registered for packet ID {}", packet_id);
        }
        
        Ok(())
    }
    
    /// Get statistics about registered handlers
    pub async fn get_stats(&self) -> HandlerStats {
        let handlers_guard = self.handlers.read().await;
        let global_guard = self.global_handlers.read().await;
        
        let packet_handler_count: usize = handlers_guard.values()
            .map(|v| v.len())
            .sum();
        
        HandlerStats {
            packet_types: handlers_guard.len(),
            packet_handlers: packet_handler_count,
            global_handlers: global_guard.len(),
        }
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler registry statistics
#[derive(Debug, Clone)]
pub struct HandlerStats {
    pub packet_types: usize,
    pub packet_handlers: usize,
    pub global_handlers: usize,
}

/// Convenience macro for creating simple packet handlers
#[macro_export]
macro_rules! packet_handler {
    ($packet_type:ty, $priority:expr, |$packet:ident, $context:ident| $body:expr) => {
        {
            struct Handler;
            
            #[async_trait::async_trait]
            impl crate::networking::handlers::system::TypedPacketHandler<$packet_type> for Handler {
                async fn handle_typed(&self, $packet: &$packet_type, $context: &crate::networking::handlers::system::HandlerContext) -> crate::networking::NetworkResult<()> {
                    $body
                }
                
                fn priority(&self) -> i32 {
                    $priority
                }
            }
            
            impl std::fmt::Debug for Handler {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "PacketHandler<{}>", std::any::type_name::<$packet_type>())
                }
            }
            
            Handler
        }
    };
}

/// Example usage of the packet handler system
#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::packets::generated::RegionHandshake;
    
    // Example typed handler
    #[derive(Debug)]
    struct RegionHandshakeHandler;
    
    #[async_trait]
    impl TypedPacketHandler<RegionHandshake> for RegionHandshakeHandler {
        async fn handle_typed(&self, packet: &RegionHandshake, context: &HandlerContext) -> NetworkResult<()> {
            debug!("Handling RegionHandshake packet");
            // Send RegionHandshakeReply
            Ok(())
        }
        
        fn priority(&self) -> i32 {
            100 // High priority
        }
    }
    
    #[tokio::test]
    async fn test_handler_registration() {
        let registry = HandlerRegistry::new();
        
        // Register handler
        registry.register_handler::<RegionHandshake, _>(RegionHandshakeHandler).await;
        
        let stats = registry.get_stats().await;
        assert_eq!(stats.packet_types, 1);
        assert_eq!(stats.packet_handlers, 1);
    }
}