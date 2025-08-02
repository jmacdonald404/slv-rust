//! Asset transfer type definitions and utilities
//! 
//! Shared types used throughout the asset transfer system.

use super::{AssetType, AssetPriority, AssetTransferStatus};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Asset metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub asset_id: Uuid,
    pub asset_type: AssetType,
    pub name: String,
    pub description: String,
    pub creator_id: Uuid,
    pub owner_id: Uuid,
    pub creation_time: std::time::SystemTime,
    pub permissions: AssetPermissions,
    pub size: Option<usize>,
    pub hash: Option<String>,
}

/// Asset permission flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPermissions {
    pub can_copy: bool,
    pub can_modify: bool,
    pub can_transfer: bool,
    pub can_damage: bool,
    pub can_fly: bool,
    pub can_push: bool,
    pub can_build: bool,
    pub can_terraform: bool,
    pub can_run_script: bool,
    pub can_deed: bool,
}

impl Default for AssetPermissions {
    fn default() -> Self {
        Self {
            can_copy: true,
            can_modify: false,
            can_transfer: true, 
            can_damage: false,
            can_fly: true,
            can_push: false,
            can_build: false,
            can_terraform: false,
            can_run_script: false,
            can_deed: false,
        }
    }
}

/// Transfer statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct TransferStats {
    pub total_requests: u64,
    pub successful_transfers: u64,
    pub failed_transfers: u64,
    pub cached_hits: u64,
    pub http_transfers: u64,
    pub udp_transfers: u64,
    pub bytes_transferred: u64,
    pub average_transfer_time: std::time::Duration,
}

impl TransferStats {
    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.successful_transfers as f64 / self.total_requests as f64) * 100.0
        }
    }
    
    /// Calculate cache hit rate as percentage  
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.cached_hits as f64 / self.total_requests as f64) * 100.0
        }
    }
}

/// Transfer progress information
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub asset_id: Uuid,
    pub bytes_received: usize,
    pub total_bytes: Option<usize>,
    pub transfer_rate: f64, // bytes per second
    pub estimated_time_remaining: Option<std::time::Duration>,
    pub status: AssetTransferStatus,
}

impl TransferProgress {
    /// Calculate completion percentage
    pub fn completion_percentage(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 {
                100.0
            } else {
                (self.bytes_received as f64 / total as f64) * 100.0
            }
        })
    }
}

/// Asset server information
#[derive(Debug, Clone)]
pub struct AssetServer {
    pub name: String,
    pub base_url: String,
    pub capabilities: HashMap<String, String>,
    pub priority: i32,
    pub max_concurrent_transfers: usize,
    pub timeout: std::time::Duration,
}

/// Transfer queue entry
#[derive(Debug, Clone)]
pub struct QueuedTransfer {
    pub asset_id: Uuid,
    pub asset_type: AssetType,
    pub priority: AssetPriority,
    pub requested_at: std::time::Instant,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl QueuedTransfer {
    pub fn new(asset_id: Uuid, asset_type: AssetType, priority: AssetPriority) -> Self {
        Self {
            asset_id,
            asset_type,
            priority,
            requested_at: std::time::Instant::now(),
            retry_count: 0,
            max_retries: 3,
        }
    }
    
    /// Check if this transfer has exceeded max retries
    pub fn has_exceeded_retries(&self) -> bool {
        self.retry_count >= self.max_retries
    }
    
    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
    
    /// Get time since this transfer was requested
    pub fn age(&self) -> std::time::Duration {
        self.requested_at.elapsed()
    }
}

/// Asset transfer configuration
#[derive(Debug, Clone)]
pub struct AssetTransferConfig {
    /// Maximum concurrent HTTP transfers
    pub max_concurrent_http: usize,
    /// Maximum concurrent UDP transfers  
    pub max_concurrent_udp: usize,
    /// HTTP request timeout
    pub http_timeout: std::time::Duration,
    /// UDP transfer timeout
    pub udp_timeout: std::time::Duration,
    /// Maximum retries per transfer
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: std::time::Duration,
    /// Cache size in MB
    pub cache_size_mb: usize,
    /// Enable aggressive caching
    pub aggressive_caching: bool,
    /// Bandwidth limit in bytes per second (0 = no limit)
    pub bandwidth_limit: usize,
}

impl Default for AssetTransferConfig {
    fn default() -> Self {
        Self {
            max_concurrent_http: 8,
            max_concurrent_udp: 4,
            http_timeout: std::time::Duration::from_secs(30),
            udp_timeout: std::time::Duration::from_secs(60),
            max_retries: 3,
            retry_delay: std::time::Duration::from_secs(2),
            cache_size_mb: 100,
            aggressive_caching: true,
            bandwidth_limit: 0, // No limit by default
        }
    }
}

/// Texture format information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    J2C,     // JPEG2000 codestream
    TGA,     // Targa
    JPEG,    // JPEG
    BMP,     // Bitmap
    PNG,     // Portable Network Graphics
}

impl TextureFormat {
    /// Get the expected file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            TextureFormat::J2C => "j2c",
            TextureFormat::TGA => "tga", 
            TextureFormat::JPEG => "jpg",
            TextureFormat::BMP => "bmp",
            TextureFormat::PNG => "png",
        }
    }
    
    /// Detect format from file header
    pub fn detect_from_header(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        
        // JPEG2000 signature
        if data.starts_with(&[0x00, 0x00, 0x00, 0x0C, 0x6A, 0x50, 0x20, 0x20]) {
            return Some(TextureFormat::J2C);
        }
        
        // JPEG signature
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(TextureFormat::JPEG);
        }
        
        // PNG signature
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Some(TextureFormat::PNG);
        }
        
        // TGA signature (at offset 0 or check footer)
        if data.len() >= 18 && data[2] == 2 {
            return Some(TextureFormat::TGA);
        }
        
        // BMP signature
        if data.starts_with(&[0x42, 0x4D]) {
            return Some(TextureFormat::BMP);
        }
        
        None
    }
}

/// Mesh format information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFormat {
    LLSD,    // Linden Lab Structured Data format
    DAE,     // COLLADA format
    OBJ,     // Wavefront OBJ format
}

impl MeshFormat {
    /// Get the expected file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            MeshFormat::LLSD => "llsd",
            MeshFormat::DAE => "dae",
            MeshFormat::OBJ => "obj",
        }
    }
}

/// Sound format information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFormat {
    WAV,     // Wave format
    OGG,     // Ogg Vorbis
    MP3,     // MPEG-1 Audio Layer 3
}

impl SoundFormat {
    /// Get the expected file extension for this format  
    pub fn extension(&self) -> &'static str {
        match self {
            SoundFormat::WAV => "wav",
            SoundFormat::OGG => "ogg", 
            SoundFormat::MP3 => "mp3",
        }
    }
    
    /// Detect format from file header
    pub fn detect_from_header(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        
        // WAV signature
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WAVE" {
            return Some(SoundFormat::WAV);
        }
        
        // OGG signature
        if data.starts_with(b"OggS") {
            return Some(SoundFormat::OGG);
        }
        
        // MP3 signature (ID3v2 or frame sync)
        if data.starts_with(b"ID3") || (data[0] == 0xFF && (data[1] & 0xE0) == 0xE0) {
            return Some(SoundFormat::MP3);
        }
        
        None
    }
}