//! Asset caching system with LRU eviction
//! 
//! Provides high-performance asset caching to reduce network transfers
//! and improve viewer responsiveness.

use super::{AssetType, AssetTransferResult};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Asset cache entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub asset_id: Uuid,
    pub asset_type: AssetType,
    pub data: Bytes,
    pub size: usize,
    pub last_accessed: std::time::Instant,
    pub access_count: u64,
}

/// LRU asset cache implementation
#[derive(Debug)]
pub struct AssetCache {
    /// Cache entries indexed by asset ID
    entries: Arc<RwLock<HashMap<Uuid, CacheEntry>>>,
    /// Maximum cache size in bytes
    max_size: usize,
    /// Current cache size in bytes
    current_size: Arc<RwLock<usize>>,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

/// Cache performance statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_requests: u64,
    pub total_bytes_cached: u64,
    pub cache_entries: usize,
}

impl CacheStats {
    /// Calculate cache hit ratio as percentage
    pub fn hit_ratio(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.hits as f64 / self.total_requests as f64) * 100.0
        }
    }
}

impl AssetCache {
    /// Create a new asset cache with specified maximum size
    pub fn new(max_size_mb: usize) -> Self {
        let max_size = max_size_mb * 1024 * 1024; // Convert MB to bytes
        
        info!("🗄️ Initializing asset cache with {} MB capacity", max_size_mb);
        
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            current_size: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }
    
    /// Get an asset from cache if available
    pub async fn get(&self, asset_id: &Uuid) -> Option<Bytes> {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        
        let mut entries = self.entries.write().await;
        
        if let Some(entry) = entries.get_mut(asset_id) {
            // Update access information
            entry.last_accessed = std::time::Instant::now();
            entry.access_count += 1;
            
            stats.hits += 1;
            debug!("📋 Cache HIT for asset {}", asset_id);
            
            Some(entry.data.clone())
        } else {
            stats.misses += 1;
            debug!("📋 Cache MISS for asset {}", asset_id);
            None
        }
    }
    
    /// Store an asset in the cache
    pub async fn put(&self, asset_id: Uuid, asset_type: AssetType, data: Bytes) {
        let size = data.len();
        
        // Check if asset already exists
        {
            let entries = self.entries.read().await;
            if entries.contains_key(&asset_id) {
                debug!("📋 Asset {} already in cache, skipping", asset_id);
                return;
            }
        }
        
        // Ensure we have space for the new asset
        self.ensure_space(size).await;
        
        let entry = CacheEntry {
            asset_id,
            asset_type,
            data,
            size,
            last_accessed: std::time::Instant::now(),
            access_count: 1,
        };
        
        // Insert the new entry
        {
            let mut entries = self.entries.write().await;
            let mut current_size = self.current_size.write().await;
            let mut stats = self.stats.write().await;
            
            entries.insert(asset_id, entry);
            *current_size += size;
            stats.total_bytes_cached += size as u64;
            stats.cache_entries = entries.len();
        }
        
        debug!("📋 Cached asset {} ({} bytes)", asset_id, size);
    }
    
    /// Remove an asset from the cache
    pub async fn remove(&self, asset_id: &Uuid) -> bool {
        let mut entries = self.entries.write().await;
        let mut current_size = self.current_size.write().await;
        let mut stats = self.stats.write().await;
        
        if let Some(entry) = entries.remove(asset_id) {
            *current_size -= entry.size;
            stats.cache_entries = entries.len();
            debug!("📋 Removed asset {} from cache", asset_id);
            true
        } else {
            false
        }
    }
    
    /// Clear all cached assets
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        let mut current_size = self.current_size.write().await;
        let mut stats = self.stats.write().await;
        
        let count = entries.len();
        entries.clear();
        *current_size = 0;
        stats.cache_entries = 0;
        
        info!("📋 Cleared {} assets from cache", count);
    }
    
    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let stats = self.stats.read().await;
        let mut stats_copy = stats.clone();
        
        // Update current cache entries count
        let entries = self.entries.read().await;
        stats_copy.cache_entries = entries.len();
        
        stats_copy
    }
    
    /// Get current cache size in bytes
    pub async fn current_size(&self) -> usize {
        *self.current_size.read().await
    }
    
    /// Check if an asset is cached
    pub async fn contains(&self, asset_id: &Uuid) -> bool {
        let entries = self.entries.read().await;
        entries.contains_key(asset_id)
    }
    
    /// Ensure there's enough space for a new asset by evicting old ones if needed
    async fn ensure_space(&self, needed_size: usize) {
        let current_size = *self.current_size.read().await;
        
        if current_size + needed_size <= self.max_size {
            return; // Enough space available
        }
        
        debug!("📋 Cache full, evicting assets to make space for {} bytes", needed_size);
        
        // Calculate how much space we need to free
        let space_needed = (current_size + needed_size) - self.max_size;
        let mut space_freed = 0;
        
        // Get list of assets sorted by LRU (least recently used first)
        let assets_to_evict = {
            let entries = self.entries.read().await;
            let mut assets: Vec<_> = entries.values().collect();
            
            // Sort by last accessed time (oldest first), then by access count (least used first)
            assets.sort_by(|a, b| {
                a.last_accessed.cmp(&b.last_accessed)
                    .then_with(|| a.access_count.cmp(&b.access_count))
            });
            
            // Select assets to evict
            let mut to_evict = Vec::new();
            for asset in assets {
                if space_freed >= space_needed {
                    break;
                }
                to_evict.push(asset.asset_id);
                space_freed += asset.size;
            }
            
            to_evict
        };
        
        // Evict selected assets
        {
            let mut entries = self.entries.write().await;
            let mut current_size = self.current_size.write().await;
            let mut stats = self.stats.write().await;
            
            for asset_id in assets_to_evict {
                if let Some(entry) = entries.remove(&asset_id) {
                    *current_size -= entry.size;
                    stats.evictions += 1;
                    debug!("📋 Evicted asset {} ({} bytes)", asset_id, entry.size);
                }
            }
            
            stats.cache_entries = entries.len();
        }
        
        info!("📋 Freed {} bytes from cache through eviction", space_freed);
    }
    
    /// Perform cache maintenance (remove expired entries, etc.)
    pub async fn maintenance(&self) {
        debug!("📋 Performing cache maintenance");
        
        let now = std::time::Instant::now();
        let max_age = std::time::Duration::from_secs(3600); // 1 hour
        
        let expired_assets = {
            let entries = self.entries.read().await;
            entries.values()
                .filter(|entry| now.duration_since(entry.last_accessed) > max_age)
                .filter(|entry| entry.access_count < 3) // Only evict rarely-used assets
                .map(|entry| entry.asset_id)
                .collect::<Vec<_>>()
        };
        
        if !expired_assets.is_empty() {
            let mut removed_count = 0;
            for asset_id in expired_assets {
                if self.remove(&asset_id).await {
                    removed_count += 1;
                }
            }
            
            if removed_count > 0 {
                info!("📋 Removed {} expired assets during maintenance", removed_count);
            }
        }
    }
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new(100) // Default 100MB cache
    }
}