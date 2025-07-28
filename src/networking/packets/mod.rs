//! Second Life packet definitions and types
//! 
//! This module provides compile-time type safety for Second Life protocol packets,
//! ensuring exact protocol compatibility while leveraging Rust's zero-cost abstractions.

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub mod types;
pub mod generated;

// Re-export all generated packets
pub use generated::*;

pub use types::*;

/// Packet frequency determines the message ID encoding size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketFrequency {
    /// High frequency: 8-bit message ID (0-254 available)
    High = 0,
    /// Medium frequency: 8-bit message ID (0-254 available) 
    Medium = 1,
    /// Low frequency: 16-bit message ID (0-32000 available)
    Low = 2,
    /// Fixed frequency: Custom message ID
    Fixed = 3,
}

/// Core packet trait that all Second Life packets must implement
/// 
/// This trait provides compile-time guarantees about packet structure
/// while maintaining exact protocol compatibility with the official viewer.
pub trait Packet: Serialize + for<'de> Deserialize<'de> + Debug + Clone + Send + Sync {
    /// Unique packet identifier within its frequency range
    const ID: u16;
    
    /// Packet frequency (determines message ID encoding)
    const FREQUENCY: PacketFrequency;
    
    /// Whether this packet should be sent reliably by default
    const RELIABLE: bool;
    
    /// Whether this packet uses zerocoding compression
    const ZEROCODED: bool;
    
    /// Whether this packet is trusted (server-to-server only)
    const TRUSTED: bool;
    
    /// Human-readable packet name for debugging
    fn name() -> &'static str;
    
    /// Get the packet's lookup key for deserialization
    fn lookup_key() -> u32 {
        match Self::FREQUENCY {
            PacketFrequency::High => Self::ID as u32,
            PacketFrequency::Medium => (1 << 16) | (Self::ID as u32),
            PacketFrequency::Low => (2 << 16) | (Self::ID as u32),
            PacketFrequency::Fixed => (3 << 16) | (Self::ID as u32),
        }
    }
}

/// Wrapper for dynamic packet handling while preserving type safety
#[derive(Debug, Clone)]
pub struct PacketWrapper {
    pub data: Vec<u8>,
    pub reliable: bool,
    pub sequence: u32,
    pub packet_id: u16,
    pub frequency: PacketFrequency,
}

impl PacketWrapper {
    pub fn new<P: Packet>(packet: &P, reliable: Option<bool>) -> crate::networking::NetworkResult<Self> {
        let data = bincode::serialize(packet)
            .map_err(|e| crate::networking::NetworkError::PacketEncode { 
                reason: e.to_string() 
            })?;
        
        Ok(PacketWrapper {
            data,
            reliable: reliable.unwrap_or(P::RELIABLE),
            sequence: 0, // Will be set by circuit
            packet_id: P::ID,
            frequency: P::FREQUENCY,
        })
    }
    
    pub fn deserialize<P: Packet>(&self) -> crate::networking::NetworkResult<P> {
        bincode::deserialize(&self.data)
            .map_err(|e| crate::networking::NetworkError::PacketDecode { 
                reason: e.to_string() 
            })
    }
}

/// Efficient packet registry for runtime dispatch
use std::collections::HashMap;
use std::sync::OnceLock;

static PACKET_REGISTRY: OnceLock<HashMap<u32, PacketInfo>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct PacketInfo {
    pub name: &'static str,
    pub id: u16,
    pub frequency: PacketFrequency,
    pub reliable: bool,
    pub zerocoded: bool,
    pub trusted: bool,
}

/// Initialize the packet registry - called once at startup
pub fn init_packet_registry() {
    let mut registry = HashMap::new();
    
    // Register all known packets
    register_packet::<UseCircuitCode>(&mut registry);
    register_packet::<CompleteAgentMovement>(&mut registry);
    register_packet::<RegionHandshake>(&mut registry);
    register_packet::<RegionHandshakeReply>(&mut registry);
    register_packet::<AgentThrottle>(&mut registry);
    register_packet::<AgentUpdate>(&mut registry);
    register_packet::<AgentHeightWidth>(&mut registry);
    register_packet::<LogoutRequest>(&mut registry);
    register_packet::<PacketAck>(&mut registry);
    register_packet::<ChatFromViewer>(&mut registry);
    
    PACKET_REGISTRY.set(registry).expect("Packet registry already initialized");
}

fn register_packet<P: Packet>(registry: &mut HashMap<u32, PacketInfo>) {
    let key = P::lookup_key();
    let info = PacketInfo {
        name: P::name(),
        id: P::ID,
        frequency: P::FREQUENCY,
        reliable: P::RELIABLE,
        zerocoded: P::ZEROCODED,
        trusted: P::TRUSTED,
    };
    registry.insert(key, info);
}

/// Look up packet information by lookup key
pub fn get_packet_info(lookup_key: u32) -> Option<&'static PacketInfo> {
    PACKET_REGISTRY.get()?.get(&lookup_key)
}

/// Look up packet information by ID and frequency
pub fn get_packet_info_by_id(id: u16, frequency: PacketFrequency) -> Option<&'static PacketInfo> {
    let key = match frequency {
        PacketFrequency::High => id as u32,
        PacketFrequency::Medium => (1 << 16) | (id as u32),
        PacketFrequency::Low => (2 << 16) | (id as u32),
        PacketFrequency::Fixed => (3 << 16) | (id as u32),
    };
    get_packet_info(key)
}