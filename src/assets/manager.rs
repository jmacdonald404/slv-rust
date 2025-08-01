use async_trait::async_trait;
use std::path::Path;
use anyhow::Result;
use crate::assets::cache::AssetCache;
use crate::assets::{Asset, texture::TextureLoader, mesh::MeshLoader};
use crate::config::{PerformanceSettingsHandle, TextureQuality};
use std::sync::Arc;
use std::collections::HashMap;
use wgpu::{Device, Queue};
use uuid::Uuid;
use tokio::sync::{RwLock, Mutex};

#[async_trait]
pub trait AssetLoader<A> {
    async fn load(&self, path: &Path) -> Result<A>;
}

/// Virtual texturing system for efficient memory management of large texture sets
pub struct VirtualTextureSystem {
    // Texture tile cache (for streaming large textures)
    tile_cache: HashMap<u64, Vec<u8>>, // tile_id -> compressed_data
    
    // Memory budget for virtual textures (in bytes)
    memory_budget: usize,
    current_usage: usize,
    
    // LRU tracking for cache eviction
    access_order: std::collections::VecDeque<u64>,
}

impl VirtualTextureSystem {
    pub fn new(memory_budget_mb: u32) -> Self {
        Self {
            tile_cache: HashMap::new(),
            memory_budget: (memory_budget_mb as usize) * 1024 * 1024,
            current_usage: 0,
            access_order: std::collections::VecDeque::new(),
        }
    }
    
    /// Request a texture tile, returns true if available in cache
    pub fn request_tile(&mut self, tile_id: u64) -> Option<&Vec<u8>> {
        if let Some(data) = self.tile_cache.get(&tile_id) {
            // Move to front of LRU
            self.access_order.retain(|&id| id != tile_id);
            self.access_order.push_front(tile_id);
            Some(data)
        } else {
            None
        }
    }
    
    /// Add a texture tile to the cache, evicting old tiles if necessary
    pub fn cache_tile(&mut self, tile_id: u64, data: Vec<u8>) {
        let data_size = data.len();
        
        // Evict old tiles if we exceed memory budget
        while self.current_usage + data_size > self.memory_budget {
            if let Some(old_tile) = self.access_order.pop_back() {
                if let Some(old_data) = self.tile_cache.remove(&old_tile) {
                    self.current_usage -= old_data.len();
                }
            } else {
                break; // No more tiles to evict
            }
        }
        
        // Add new tile
        self.tile_cache.insert(tile_id, data);
        self.current_usage += data_size;
        self.access_order.push_front(tile_id);
    }
    
    /// Get current memory usage statistics
    pub fn get_memory_stats(&self) -> (usize, usize, usize) {
        (self.current_usage, self.memory_budget, self.tile_cache.len())
    }
}

/// Enhanced resource manager with UUID-based asset loading and performance settings integration
pub struct ResourceManager {
    // Traditional path-based cache for local assets
    pub path_cache: AssetCache<String, Asset>,
    
    // UUID-based cache for Second Life assets  
    pub uuid_cache: Arc<RwLock<HashMap<Uuid, Asset>>>,
    
    // Asset loaders
    pub texture_loader: TextureLoader,
    pub mesh_loader: MeshLoader,
    
    // Performance settings for dynamic quality adjustment
    performance_settings: Option<PerformanceSettingsHandle>,
    
    // Asset request tracking for network loading
    pending_requests: Arc<RwLock<HashMap<Uuid, tokio::time::Instant>>>,
    
    // Virtual texturing system for managing large texture sets
    pub virtual_texture_system: Arc<Mutex<VirtualTextureSystem>>,
}

impl ResourceManager {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self::new_with_settings(device, queue, None)
    }
    
    pub fn new_with_settings(device: Arc<Device>, queue: Arc<Queue>, performance_settings: Option<PerformanceSettingsHandle>) -> Self {
        // Determine VT memory budget based on performance settings
        let vt_memory_mb = if let Some(ref settings) = performance_settings {
            if let Ok(settings) = settings.read() {
                settings.memory.texture_cache_size_mb
            } else {
                512 // Default fallback
            }
        } else {
            512 // Default when no settings provided
        };
        
        Self {
            path_cache: AssetCache::new(),
            uuid_cache: Arc::new(RwLock::new(HashMap::new())),
            texture_loader: TextureLoader::new(Arc::clone(&device), Arc::clone(&queue)),
            mesh_loader: MeshLoader::new(device),
            performance_settings,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            virtual_texture_system: Arc::new(Mutex::new(VirtualTextureSystem::new(vt_memory_mb))),
        }
    }

    // Path-based loading (existing functionality)
    pub async fn load_texture(&mut self, path: &Path) -> anyhow::Result<()> {
        let texture = self.texture_loader.load(path).await?;
        self.path_cache.insert(path.to_str().unwrap().to_string(), Asset::Texture(texture));
        Ok(())
    }

    pub async fn load_mesh(&mut self, path: &Path) -> anyhow::Result<()> {
        let mesh = self.mesh_loader.load(path).await?;
        self.path_cache.insert(path.to_str().unwrap().to_string(), Asset::Mesh(mesh));
        Ok(())
    }

    pub fn get_texture(&self, path: &str) -> Option<&crate::assets::texture::Texture> {
        match self.path_cache.get(&path.to_string()) {
            Some(Asset::Texture(t)) => Some(t),
            _ => None,
        }
    }

    pub fn get_mesh(&self, path: &str) -> Option<&crate::assets::mesh::Mesh> {
        match self.path_cache.get(&path.to_string()) {
            Some(Asset::Mesh(m)) => Some(m),
            _ => None,
        }
    }

    // UUID-based loading for Second Life assets
    pub async fn load_texture_by_uuid(&self, texture_id: Uuid) -> anyhow::Result<()> {
        // Mark as pending to avoid duplicate requests
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(texture_id, tokio::time::Instant::now());
        }
        
        // In a real implementation, this would trigger a network request
        // For now, we'll simulate loading a placeholder texture
        println!("Requesting texture with UUID: {}", texture_id);
        
        // TODO: Send NetworkCommand::RequestTexture through channel
        Ok(())
    }

    pub async fn load_mesh_by_uuid(&self, mesh_id: Uuid) -> anyhow::Result<()> {
        // Mark as pending to avoid duplicate requests
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(mesh_id, tokio::time::Instant::now());
        }
        
        // In a real implementation, this would trigger a network request
        println!("Requesting mesh with UUID: {}", mesh_id);
        
        // TODO: Send NetworkCommand::RequestObject through channel
        Ok(())
    }

    pub async fn get_texture_by_uuid(&self, texture_id: &Uuid) -> Option<crate::assets::texture::Texture> {
        let cache = self.uuid_cache.read().await;
        match cache.get(texture_id) {
            Some(Asset::Texture(texture)) => Some(texture.clone()),
            _ => None,
        }
    }

    pub async fn get_mesh_by_uuid(&self, mesh_id: &Uuid) -> Option<crate::assets::mesh::Mesh> {
        let cache = self.uuid_cache.read().await;
        match cache.get(mesh_id) {
            Some(Asset::Mesh(mesh)) => Some(mesh.clone()),
            _ => None,
        }
    }

    /// Store an asset received from the network by UUID
    pub async fn store_asset_by_uuid(&self, uuid: Uuid, asset: Asset) {
        let mut cache = self.uuid_cache.write().await;
        cache.insert(uuid, asset);
        
        // Remove from pending requests
        let mut pending = self.pending_requests.write().await;
        pending.remove(&uuid);
    }

    /// Check if an asset request is already pending
    pub async fn is_request_pending(&self, uuid: &Uuid) -> bool {
        let pending = self.pending_requests.read().await;
        pending.contains_key(uuid)
    }

    /// Get current cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize, usize) {
        let uuid_cache = self.uuid_cache.read().await;
        let pending = self.pending_requests.read().await;
        let vt_system = self.virtual_texture_system.lock().await;
        let vt_stats = vt_system.get_memory_stats();
        
        (uuid_cache.len(), pending.len(), vt_stats.0) // uuid_cached, pending, vt_memory_usage
    }

    /// Clean up expired pending requests (called periodically)
    pub async fn cleanup_expired_requests(&self, timeout_seconds: u64) {
        let mut pending = self.pending_requests.write().await;
        let now = tokio::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_seconds);
        
        pending.retain(|_, &mut start_time| now.duration_since(start_time) < timeout);
    }

    /// Request a virtual texture tile
    pub async fn request_vt_tile(&self, tile_id: u64) -> Option<Vec<u8>> {
        let mut vt_system = self.virtual_texture_system.lock().await;
        vt_system.request_tile(tile_id).cloned()
    }

    /// Cache a virtual texture tile
    pub async fn cache_vt_tile(&self, tile_id: u64, data: Vec<u8>) {
        let mut vt_system = self.virtual_texture_system.lock().await;
        vt_system.cache_tile(tile_id, data);
    }
}
