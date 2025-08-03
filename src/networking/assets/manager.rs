//! Unified asset manager that combines HTTP and UDP transfers
//! 
//! This is the main interface for asset transfers in the Second Life viewer,
//! providing a unified API for both HTTP capability-based and UDP packet-based
//! asset transfers as specified in netplan.md.

use super::{
    AssetTransferRequest, AssetTransferResult, AssetTransferStatus, AssetTransferMethod,
    AssetType, AssetPriority, AssetTransferCallback, AssetCache,
    types::{AssetTransferConfig, TransferStats, QueuedTransfer, TransferProgress}
};
use crate::networking::{NetworkResult, NetworkError};
use crate::networking::capabilities::CapabilitiesManager;
use crate::networking::packets::generated::*;
use crate::networking::circuit::Circuit;
use bytes::Bytes;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex, Semaphore};
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// Unified asset manager
pub struct AssetManager {
    /// Asset cache for reducing network transfers
    cache: AssetCache,
    /// Capabilities manager for HTTP transfers
    capabilities: Option<Arc<CapabilitiesManager>>,
    /// Active circuits for UDP transfers
    circuits: Arc<RwLock<HashMap<Uuid, Arc<Circuit>>>>,
    /// Transfer configuration
    config: AssetTransferConfig,
    /// Transfer queue sorted by priority
    transfer_queue: Arc<Mutex<VecDeque<QueuedTransfer>>>,
    /// Active transfers
    active_transfers: Arc<RwLock<HashMap<Uuid, TransferProgress>>>,
    /// HTTP transfer semaphore for concurrency control
    http_semaphore: Arc<Semaphore>,
    /// UDP transfer semaphore for concurrency control  
    udp_semaphore: Arc<Semaphore>,
    /// Transfer statistics
    stats: Arc<RwLock<TransferStats>>,
    /// Callbacks for completed transfers
    callbacks: Arc<RwLock<HashMap<Uuid, AssetTransferCallback>>>,
}

impl std::fmt::Debug for AssetManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetManager")
            .field("config", &self.config)
            .field("capabilities", &self.capabilities.as_ref().map(|_| "Some(CapabilitiesManager)"))
            .field("circuits_count", &"circuits")
            .field("active_transfers_count", &"active_transfers")
            .finish()
    }
}

impl AssetManager {
    /// Create a new asset manager
    pub fn new(config: AssetTransferConfig) -> Self {
        let cache = AssetCache::new(config.cache_size_mb);
        
        info!("🎨 Initializing AssetManager with {} MB cache", config.cache_size_mb);
        
        Self {
            cache,
            capabilities: None,
            circuits: Arc::new(RwLock::new(HashMap::new())),
            http_semaphore: Arc::new(Semaphore::new(config.max_concurrent_http)),
            udp_semaphore: Arc::new(Semaphore::new(config.max_concurrent_udp)),
            config,
            transfer_queue: Arc::new(Mutex::new(VecDeque::new())),
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(TransferStats::default())),
            callbacks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Set the capabilities manager for HTTP transfers
    pub async fn set_capabilities_manager(&mut self, capabilities: Arc<CapabilitiesManager>) {
        info!("🌐 Asset manager now using HTTP capabilities");
        self.capabilities = Some(capabilities);
    }
    
    /// Add a circuit for UDP transfers
    pub async fn add_circuit(&self, region_id: Uuid, circuit: Arc<Circuit>) {
        let mut circuits = self.circuits.write().await;
        circuits.insert(region_id, circuit);
        info!("📡 Added circuit for region {}", region_id);
    }
    
    /// Remove a circuit
    pub async fn remove_circuit(&self, region_id: &Uuid) {
        let mut circuits = self.circuits.write().await;
        circuits.remove(region_id);
        info!("📡 Removed circuit for region {}", region_id);
    }
    
    /// Request an asset transfer
    pub async fn request_asset(&self, request: AssetTransferRequest) -> NetworkResult<()> {
        let asset_id = request.asset_id;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }
        
        // Check cache first
        if let Some(cached_data) = self.cache.get(&asset_id).await {
            info!("📋 Asset {} found in cache", asset_id);
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.cached_hits += 1;
            }
            
            // Call callback with cached result
            if let Some(callback) = &request.callback {
                let result = AssetTransferResult {
                    asset_id,
                    asset_type: request.asset_type,
                    status: AssetTransferStatus::Cached,
                    data: Some(cached_data),
                    error: None,
                    transfer_time: Duration::from_millis(0),
                };
                callback(result);
            }
            
            return Ok(());
        }
        
        // Store callback if provided
        if let Some(callback) = request.callback {
            let mut callbacks = self.callbacks.write().await;
            callbacks.insert(asset_id, callback);
        }
        
        // Add to transfer queue
        let queued_transfer = QueuedTransfer::new(asset_id, request.asset_type, request.priority);
        {
            let mut queue = self.transfer_queue.lock().await;
            
            // Insert in priority order (higher priority first)
            let insert_pos = queue.iter().position(|item| item.priority < request.priority)
                .unwrap_or(queue.len());
            queue.insert(insert_pos, queued_transfer);
        }
        
        // Process the queue
        // Note: This would need to be called on an Arc<AssetManager> 
        // For now, we'll just add to queue and process later
        
        Ok(())
    }
    
    /// Process pending transfers in the queue
    fn process_transfer_queue(self: &Arc<Self>, preferred_method: AssetTransferMethod) {
        let queued_transfer = {
            let queue_clone = Arc::clone(&self.transfer_queue);
            tokio::spawn(async move {
                let mut queue = queue_clone.lock().await;
                queue.pop_front()
            })
        };
        
        let manager_clone = Arc::clone(self);
        tokio::spawn(async move {
            if let Ok(Some(transfer)) = queued_transfer.await {
                manager_clone.execute_transfer(transfer, preferred_method).await;
            }
        });
    }
    
    /// Execute a single asset transfer
    async fn execute_transfer(&self, mut transfer: QueuedTransfer, method: AssetTransferMethod) {
        let start_time = std::time::Instant::now();
        
        // Create transfer progress entry
        {
            let mut active = self.active_transfers.write().await;
            active.insert(transfer.asset_id, TransferProgress {
                asset_id: transfer.asset_id,
                bytes_received: 0,
                total_bytes: None,
                transfer_rate: 0.0,
                estimated_time_remaining: None,
                status: AssetTransferStatus::InProgress,
            });
        }
        
        let result = match method {
            AssetTransferMethod::Http => {
                self.transfer_via_http(&transfer).await
            },
            AssetTransferMethod::Udp => {
                self.transfer_via_udp(&transfer).await
            },
            AssetTransferMethod::HttpWithUdpFallback => {
                // Try HTTP first
                match self.transfer_via_http(&transfer).await {
                    Ok(data) => Ok(data),
                    Err(_) => {
                        debug!("🔄 HTTP transfer failed for {}, trying UDP fallback", transfer.asset_id);
                        self.transfer_via_udp(&transfer).await
                    }
                }
            }
        };
        
        let transfer_time = start_time.elapsed();
        
        // Remove from active transfers
        {
            let mut active = self.active_transfers.write().await;
            active.remove(&transfer.asset_id);
        }
        
        // Process result
        let transfer_result = match result {
            Ok(data) => {
                // Cache the asset
                self.cache.put(transfer.asset_id, transfer.asset_type, data.clone()).await;
                
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.successful_transfers += 1;
                    stats.bytes_transferred += data.len() as u64;
                    
                    // Update average transfer time
                    let total_time = stats.average_transfer_time.as_millis() as u64 * stats.successful_transfers;
                    let new_avg = (total_time + transfer_time.as_millis() as u64) / (stats.successful_transfers + 1);
                    stats.average_transfer_time = Duration::from_millis(new_avg);
                }
                
                AssetTransferResult {
                    asset_id: transfer.asset_id,
                    asset_type: transfer.asset_type,
                    status: AssetTransferStatus::Success,
                    data: Some(data),
                    error: None,
                    transfer_time,
                }
            },
            Err(error) => {
                // Handle retry logic
                if !transfer.has_exceeded_retries() {
                    transfer.increment_retry();
                    
                    debug!("🔄 Retrying transfer for {} (attempt {}/{})", 
                           transfer.asset_id, transfer.retry_count, transfer.max_retries);
                    
                    // Add back to queue with delay
                    let queue_clone = Arc::clone(&self.transfer_queue);
                    let retry_delay = self.config.retry_delay;
                    tokio::spawn(async move {
                        tokio::time::sleep(retry_delay).await;
                        let mut queue = queue_clone.lock().await;
                        queue.push_back(transfer);
                    });
                    
                    return; // Don't call callback yet
                }
                
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.failed_transfers += 1;
                }
                
                AssetTransferResult {
                    asset_id: transfer.asset_id,
                    asset_type: transfer.asset_type,
                    status: AssetTransferStatus::Failed,
                    data: None,
                    error: Some(error.to_string()),
                    transfer_time,
                }
            }
        };
        
        // Call callback if registered
        let callback = {
            let mut callbacks = self.callbacks.write().await;
            callbacks.remove(&transfer.asset_id)
        };
        
        if let Some(callback) = callback {
            callback(transfer_result);
        }
    }
    
    /// Transfer asset via HTTP capabilities
    async fn transfer_via_http(&self, transfer: &QueuedTransfer) -> NetworkResult<Bytes> {
        let _permit = self.http_semaphore.acquire().await
            .map_err(|e| NetworkError::Other { reason: e.to_string() })?;
        
        let capabilities = self.capabilities.as_ref()
            .ok_or_else(|| NetworkError::Other { reason: "No capabilities manager available".to_string() })?;
        
        debug!("🌐 Starting HTTP transfer for asset {}", transfer.asset_id);
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.http_transfers += 1;
        }
        
        let data = match transfer.asset_type {
            AssetType::Texture | AssetType::ImageJPEG | AssetType::ImageTGA => {
                timeout(
                    self.config.http_timeout,
                    capabilities.get_texture(transfer.asset_id)
                ).await
                .map_err(|_| NetworkError::Other { reason: "HTTP transfer timeout".to_string() })?
                .map_err(|e| NetworkError::Other { reason: e.to_string() })?
            },
            AssetType::Mesh => {
                timeout(
                    self.config.http_timeout,
                    capabilities.get_mesh(transfer.asset_id)
                ).await
                .map_err(|_| NetworkError::Other { reason: "HTTP transfer timeout".to_string() })?
                .map_err(|e| NetworkError::Other { reason: e.to_string() })?
            },
            _ => {
                return Err(NetworkError::Other { 
                    reason: format!("Asset type {:?} not supported for HTTP transfer", transfer.asset_type) 
                });
            }
        };
        
        info!("✅ ASSET RESPONSE: HTTP transfer completed successfully");
        info!("   Asset ID: {}", transfer.asset_id);
        info!("   Asset Type: {:?}", transfer.asset_type);
        info!("   Data size: {} bytes", data.len());
        info!("   Transfer method: HTTP capability");
        Ok(Bytes::from(data))
    }
    
    /// Transfer asset via UDP packets
    async fn transfer_via_udp(&self, transfer: &QueuedTransfer) -> NetworkResult<Bytes> {
        let _permit = self.udp_semaphore.acquire().await
            .map_err(|e| NetworkError::Other { reason: e.to_string() })?;
        
        debug!("📡 Starting UDP transfer for asset {}", transfer.asset_id);
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.udp_transfers += 1;
        }
        
        // For now, return an error as UDP transfer implementation would require
        // significant packet handling logic that's beyond the scope of this integration
        // In a full implementation, this would:
        // 1. Send TransferRequest packet
        // 2. Handle TransferInfo response
        // 3. Collect TransferPacket responses
        // 4. Reassemble the asset data
        
        warn!("📡 UDP asset transfer not yet fully implemented for asset {}", transfer.asset_id);
        Err(NetworkError::Other { 
            reason: "UDP asset transfer implementation pending".to_string() 
        })
    }
    
    /// Get current transfer statistics
    pub async fn get_stats(&self) -> TransferStats {
        self.stats.read().await.clone()
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> super::cache::CacheStats {
        self.cache.stats().await
    }
    
    /// Get active transfer progress
    pub async fn get_active_transfers(&self) -> Vec<TransferProgress> {
        let active = self.active_transfers.read().await;
        active.values().cloned().collect()
    }
    
    /// Cancel an active transfer
    pub async fn cancel_transfer(&self, asset_id: &Uuid) -> bool {
        // Remove from active transfers
        let was_active = {
            let mut active = self.active_transfers.write().await;
            active.remove(asset_id).is_some()
        };
        
        // Remove from queue
        let was_queued = {
            let mut queue = self.transfer_queue.lock().await;
            let original_len = queue.len();
            queue.retain(|item| item.asset_id != *asset_id);
            queue.len() != original_len
        };
        
        // Remove callback
        {
            let mut callbacks = self.callbacks.write().await;
            callbacks.remove(asset_id);
        }
        
        if was_active || was_queued {
            info!("❌ Cancelled transfer for asset {}", asset_id);
            true
        } else {
            false
        }
    }
    
    /// Clear the asset cache
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }
    
    /// Perform maintenance tasks
    pub async fn maintenance(&self) {
        debug!("🔧 Performing asset manager maintenance");
        
        // Cache maintenance
        self.cache.maintenance().await;
        
        // Clean up old queued transfers
        {
            let mut queue = self.transfer_queue.lock().await;
            let max_age = Duration::from_secs(300); // 5 minutes
            queue.retain(|item| item.age() < max_age);
        }
        
        // Clean up stale active transfers
        {
            let mut active = self.active_transfers.write().await;
            let max_age = Duration::from_secs(120); // 2 minutes
            let now = std::time::Instant::now();
            
            active.retain(|_, progress| {
                // Keep transfers that are still active (this is a simplified check)
                progress.status == AssetTransferStatus::InProgress
            });
        }
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new(AssetTransferConfig::default())
    }
}