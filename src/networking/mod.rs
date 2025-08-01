//! Second Life networking implementation
//! 
//! This module provides a high-performance, protocol-compliant implementation
//! of the Second Life UDP protocol, leveraging Rust's strengths in memory safety,
//! zero-cost abstractions, and fearless concurrency.

pub mod client;
pub mod core;
pub mod circuit;
pub mod packets;
pub mod serialization;
pub mod handlers;
pub mod transport;
pub mod socks5_udp;
pub mod auth;
pub mod manager;
pub mod proxy;

// Re-export main types for convenience
pub use client::Client;
pub use core::Core;
pub use circuit::Circuit;
pub use packets::{Packet, PacketFrequency};
pub use serialization::{PacketSerializer, PacketDeserializer};
pub use transport::UdpTransport;

// Error types
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum NetworkError {
    #[error("Connection lost to {address}")]
    ConnectionLost { address: std::net::SocketAddr },
    
    #[error("Packet decode failed: {reason}")]
    PacketDecode { reason: String },
    
    #[error("Packet encode failed: {reason}")]
    PacketEncode { reason: String },
    
    #[error("Circuit not found: {id}")]
    CircuitNotFound { id: u32 },
    
    #[error("Handshake timeout")]
    HandshakeTimeout,
    
    #[error("Handshake failed: {reason}")]
    HandshakeFailed { reason: String },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Login server rejected credentials: {reason}")]
    LoginRejected { reason: String },
    
    #[error("Simulator connection failed: {reason}")]
    SimulatorConnectionFailed { reason: String },
    
    #[error("Region handshake failed: {reason}")]
    RegionHandshakeFailed { reason: String },
    
    #[error("Transport error: {reason}")]
    Transport { reason: String },

    #[error("{reason}")]
    Other { reason: String },
}

pub type NetworkResult<T> = Result<T, NetworkError>;

impl From<std::io::Error> for NetworkError {
    fn from(err: std::io::Error) -> Self {
        NetworkError::Transport { reason: err.to_string() }
    }
}
