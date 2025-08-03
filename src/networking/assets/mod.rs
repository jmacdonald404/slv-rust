//! Integrated asset transfer system for Second Life
//! 
//! This module provides a unified asset management system that combines
//! both UDP packet-based transfers and HTTP capability-based transfers
//! as specified in netplan.md under "Application-Layer Protocols".

use crate::networking::{NetworkResult, NetworkError};
use crate::networking::capabilities::{CapabilitiesManager, CapabilityError};
use crate::networking::packets::generated::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, info, warn, error};
use uuid::Uuid;
use bytes::Bytes;

pub mod cache;
pub mod manager;
pub mod types;

pub use manager::AssetManager;
pub use types::*;
pub use cache::AssetCache;

/// Asset transfer methods supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetTransferMethod {
    /// Use UDP packets (TransferRequest/TransferInfo/TransferPacket)
    Udp,
    /// Use HTTP capabilities (GetTexture, GetMesh2, etc.)
    Http,
    /// Try HTTP first, fallback to UDP
    HttpWithUdpFallback,
}

/// Asset transfer request
#[derive(Clone)]
pub struct AssetTransferRequest {
    pub asset_id: Uuid,
    pub asset_type: AssetType,
    pub priority: AssetPriority,
    pub method: AssetTransferMethod,
    pub callback: Option<AssetTransferCallback>,
}

impl std::fmt::Debug for AssetTransferRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetTransferRequest")
            .field("asset_id", &self.asset_id)
            .field("asset_type", &self.asset_type)
            .field("priority", &self.priority)
            .field("method", &self.method)
            .field("callback", &self.callback.as_ref().map(|_| "Some(callback)"))
            .finish()
    }
}

/// Asset transfer callback type
pub type AssetTransferCallback = Arc<dyn Fn(AssetTransferResult) + Send + Sync>;

/// Result of an asset transfer
#[derive(Debug, Clone)]
pub struct AssetTransferResult {
    pub asset_id: Uuid,
    pub asset_type: AssetType,
    pub status: AssetTransferStatus,
    pub data: Option<Bytes>,
    pub error: Option<String>,
    pub transfer_time: std::time::Duration,
}

/// Status of an asset transfer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetTransferStatus {
    /// Transfer completed successfully
    Success,
    /// Transfer failed with error
    Failed,
    /// Transfer was cancelled
    Cancelled,
    /// Transfer is in progress
    InProgress,
    /// Asset not found
    NotFound,
    /// Asset found in cache
    Cached,
}

/// Asset priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Asset types supported by the transfer system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetType {
    Texture = 0,
    Sound = 1,
    CallingCard = 2,
    Landmark = 3,
    Script = 4,
    Clothing = 5,
    Object = 6,
    Notecard = 7,
    Category = 8,
    RootCategory = 9,
    LSLText = 10,
    LSLBytecode = 11,
    TextureTGA = 12,
    Bodypart = 13,
    TrashFolder = 14,
    SnapshotFolder = 15,
    LostAndFoundFolder = 16,
    SoundWAV = 17,
    ImageTGA = 18,
    ImageJPEG = 19,
    Animation = 20,
    Gesture = 21,
    Simstate = 22,
    FavoriteFolder = 23,
    Link = 24,
    LinkFolder = 25,
    MarketplaceFolder = 46,
    Mesh = 49,
    Settings = 56,
    Material = 57,
}

impl AssetType {
    /// Check if this asset type supports HTTP capability transfers
    pub fn supports_http_transfer(&self) -> bool {
        matches!(self, 
            AssetType::Texture | 
            AssetType::ImageJPEG | 
            AssetType::ImageTGA |
            AssetType::Mesh
        )
    }
    
    /// Get the capability name for HTTP transfers
    pub fn http_capability_name(&self) -> Option<&'static str> {
        match self {
            AssetType::Texture | AssetType::ImageJPEG | AssetType::ImageTGA => {
                Some(crate::networking::capabilities::well_known_capabilities::GET_TEXTURE)
            },
            AssetType::Mesh => {
                Some(crate::networking::capabilities::well_known_capabilities::GET_MESH)
            },
            _ => None,
        }
    }
}

impl From<u8> for AssetType {
    fn from(value: u8) -> Self {
        match value {
            0 => AssetType::Texture,
            1 => AssetType::Sound,
            2 => AssetType::CallingCard,
            3 => AssetType::Landmark,
            4 => AssetType::Script,
            5 => AssetType::Clothing,
            6 => AssetType::Object,
            7 => AssetType::Notecard,
            8 => AssetType::Category,
            9 => AssetType::RootCategory,
            10 => AssetType::LSLText,
            11 => AssetType::LSLBytecode,
            12 => AssetType::TextureTGA,
            13 => AssetType::Bodypart,
            14 => AssetType::TrashFolder,
            15 => AssetType::SnapshotFolder,
            16 => AssetType::LostAndFoundFolder,
            17 => AssetType::SoundWAV,
            18 => AssetType::ImageTGA,
            19 => AssetType::ImageJPEG,
            20 => AssetType::Animation,
            21 => AssetType::Gesture,
            22 => AssetType::Simstate,
            23 => AssetType::FavoriteFolder,
            24 => AssetType::Link,
            25 => AssetType::LinkFolder,
            46 => AssetType::MarketplaceFolder,
            49 => AssetType::Mesh,
            56 => AssetType::Settings,
            57 => AssetType::Material,
            _ => AssetType::Object, // Default fallback
        }
    }
}

/// Convert NetworkError to CapabilityError for unified error handling
impl From<NetworkError> for CapabilityError {
    fn from(err: NetworkError) -> Self {
        CapabilityError::HttpError(err.to_string())
    }
}

/// Convert CapabilityError to NetworkError for unified error handling
impl From<CapabilityError> for NetworkError {
    fn from(err: CapabilityError) -> Self {
        NetworkError::Other { reason: err.to_string() }
    }
}