//! Asset transfer system packet handlers
//! 
//! This module handles packets related to asset transfers including textures,
//! meshes, sounds, and other asset types per netplan.md requirements.
//! Now integrated with the unified AssetManager system.

use crate::networking::{NetworkResult};
use crate::networking::handlers::{TypedPacketHandler, HandlerContext};
use crate::networking::packets::{generated::*};
use crate::networking::assets::{
    AssetManager, AssetTransferRequest, AssetType, AssetPriority, 
    AssetTransferMethod, AssetTransferCallback
};
use tracing::{info, debug, warn, error};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// Handler for TransferRequest packets - initiates asset transfers
pub struct TransferRequestHandler {
    asset_manager: Arc<AssetManager>,
}

impl TransferRequestHandler {
    pub fn new(asset_manager: Arc<AssetManager>) -> Self {
        Self { asset_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<TransferRequest> for TransferRequestHandler {
    async fn handle_typed(&self, transfer_request: TransferRequest, _context: &HandlerContext) -> NetworkResult<()> {
        info!("📥 Received TransferRequest");
        
        debug!("🔄 Transfer ID: {}", transfer_request.transfer_id);
        debug!("   Channel Type: {}, Source Type: {}", 
               transfer_request.channel_type,
               transfer_request.source_type);
        debug!("   Priority: {}", transfer_request.priority);
        
        // Process transfer parameters to extract asset ID
        if !transfer_request.params.data.is_empty() && transfer_request.params.data.len() >= 16 {
            // Parse asset ID from parameters (simplified - real implementation would parse LLSD)
            let asset_id_bytes: [u8; 16] = transfer_request.params.data[0..16].try_into()
                    .map_err(|_| crate::networking::NetworkError::PacketDecode { 
                        reason: "Invalid asset ID in transfer parameters".to_string() 
                    })?;
                
                let asset_id = Uuid::from_bytes(asset_id_bytes);
                let asset_type = AssetType::from(transfer_request.source_type as u8);
                let priority = match transfer_request.priority {
                    0.0..=25.0 => AssetPriority::Low,
                    26.0..=75.0 => AssetPriority::Normal,
                    76.0..=100.0 => AssetPriority::High,
                    _ => AssetPriority::Critical,
                };
                
                // Create asset transfer request
                let asset_request = AssetTransferRequest {
                    asset_id,
                    asset_type,
                    priority,
                    method: AssetTransferMethod::HttpWithUdpFallback,
                    callback: None, // Could add callback for transfer completion notifications
                };
                
                // Submit to asset manager
                if let Err(e) = self.asset_manager.request_asset(asset_request).await {
                    error!("❌ Failed to request asset {}: {}", asset_id, e);
                } else {
                    info!("✅ Queued asset transfer for {}", asset_id);
                }
        }
        
        debug!("✅ Processed transfer request: {}", transfer_request.transfer_id);
        Ok(())
    }
}

/// Handler for TransferInfo packets - provides transfer status and data
pub struct TransferInfoHandler;

impl TransferInfoHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<TransferInfo> for TransferInfoHandler {
    async fn handle_typed(&self, transfer_info: TransferInfo, _context: &HandlerContext) -> NetworkResult<()> {
        info!("📤 Received TransferInfo");
        
        debug!("🔄 Transfer ID: {}", transfer_info.transfer_id);
        debug!("   Channel Type: {}, Target Type: {}", 
               transfer_info.channel_type,
               transfer_info.target_type);
        debug!("   Status: {}, Size: {}", 
               transfer_info.status,
               transfer_info.size);
        
        // TODO: Process transfer data if present
        // Handle different transfer types
        match transfer_info.channel_type {
            0 => debug!("   Channel: Misc"),
            1 => debug!("   Channel: Asset"),
            2 => debug!("   Channel: Estate"),
            _ => debug!("   Channel: Unknown ({})", transfer_info.channel_type),
        }
        
        // TODO: Process asset data based on transfer type
        // TODO: Update asset cache with received data
        // TODO: Emit asset transfer events per netplan.md
        
        debug!("✅ Processed transfer info: {}", transfer_info.transfer_id);
        Ok(())
    }
}

/// Handler for TransferAbort packets - handles transfer cancellations
pub struct TransferAbortHandler;

impl TransferAbortHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<TransferAbort> for TransferAbortHandler {
    async fn handle_typed(&self, transfer_abort: TransferAbort, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🚫 Received TransferAbort");
        
        debug!("🔄 Transfer ID: {}", transfer_abort.transfer_id);
        
        // TODO: Cancel ongoing transfer
        // TODO: Clean up transfer resources
        // TODO: Emit transfer cancelled event per netplan.md
        
        debug!("✅ Processed transfer abort: {}", transfer_abort.transfer_id);
        Ok(())
    }
}

/// Handler for RequestImage packets - requests texture assets
pub struct RequestImageHandler {
    asset_manager: Arc<AssetManager>,
}

impl RequestImageHandler {
    pub fn new(asset_manager: Arc<AssetManager>) -> Self {
        Self { asset_manager }
    }
}

#[async_trait]
impl TypedPacketHandler<RequestImage> for RequestImageHandler {
    async fn handle_typed(&self, image_request: RequestImage, _context: &HandlerContext) -> NetworkResult<()> {
        info!("🖼️ Received RequestImage with {} requests", image_request.request_image.len());
        
        // Process image requests
        for request_image in &image_request.request_image {
            debug!("🖼️ Requesting image: {}", request_image.image);
            debug!("   Discard Level: {}, Packet: {}", 
                   request_image.discard_level, request_image.packet);
            debug!("   Download Priority: {}", request_image.download_priority);
            
            // Map download priority to asset priority
            let priority = match request_image.download_priority {
                0.0..=0.25 => AssetPriority::Low,
                0.26..=0.75 => AssetPriority::Normal,
                0.76..=1.0 => AssetPriority::High,
                _ => AssetPriority::Critical,
            };
            
            // Create asset transfer request for texture
            let asset_request = AssetTransferRequest {
                asset_id: request_image.image,
                asset_type: AssetType::Texture,
                priority,
                method: AssetTransferMethod::HttpWithUdpFallback,
                callback: None, // Could add callback to update texture rendering
            };
            
            // Submit to asset manager
            if let Err(e) = self.asset_manager.request_asset(asset_request).await {
                error!("❌ Failed to request texture {}: {}", request_image.image, e);
            } else {
                debug!("✅ Queued texture download for {}", request_image.image);
            }
        }
        
        info!("✅ Processed {} texture requests", image_request.request_image.len());
        Ok(())
    }
}

/// Handler for ImageData packets - receives texture data
pub struct ImageDataHandler;

impl ImageDataHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<ImageData> for ImageDataHandler {
    async fn handle_typed(&self, image_data: ImageData, _context: &HandlerContext) -> NetworkResult<()> {
        debug!("🖼️ Received ImageData: {} bytes", image_data.data.data.len());
        
        debug!("🖼️ Image ID: {}", image_data.id);
        debug!("   Packets: {}, Codec: {}", 
               image_data.packets, image_data.codec);
        debug!("   Size: {}x{}", image_data.size, image_data.size);
        
        // TODO: Process image data based on codec
        // TODO: Reassemble multi-packet images
        // TODO: Update texture cache
        // TODO: Emit texture loaded event per netplan.md
        
        Ok(())
    }
}

/// Handler for LayerData packets (terrain data)
pub struct LayerDataHandler;

impl LayerDataHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TypedPacketHandler<LayerData> for LayerDataHandler {
    async fn handle_typed(&self, layer_data: LayerData, _context: &HandlerContext) -> NetworkResult<()> {
        debug!("🗺️ Received LayerData");
        
        debug!("🗺️ Layer Type: {}", layer_data.r#type);
        debug!("   Data: {} bytes", layer_data.data.data.len());
        
        // Process different layer types
        match layer_data.r#type {
            0 => debug!("   Layer: Land (terrain height)"),
            1 => debug!("   Layer: Wind"),
            2 => debug!("   Layer: Cloud"),
            _ => debug!("   Layer: Unknown type {}", layer_data.r#type),
        }
        
        // TODO: Process terrain/layer data
        // TODO: Update terrain renderer with new data
        // TODO: Emit terrain update event per netplan.md
        
        Ok(())
    }
}