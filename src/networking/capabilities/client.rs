//! HTTP client for Second Life capabilities
//! 
//! This module provides high-level client functions for common
//! capability operations in Second Life.

use super::{CapabilitiesManager, CapabilityMethod, CapabilityError, well_known_capabilities};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, warn};
use uuid::Uuid;

impl CapabilitiesManager {
    /// Fetch inventory using the FetchInventory2 capability
    pub async fn fetch_inventory(&self, folder_id: Uuid) -> Result<InventoryResponse, CapabilityError> {
        let request_body = json!({
            "folder_id": folder_id,
            "fetch_folders": true,
            "fetch_items": true,
            "sort_order": 1
        });

        let response = self.capability_request(
            well_known_capabilities::FETCH_INVENTORY,
            CapabilityMethod::Post,
            Some(request_body),
        ).await?;

        let inventory: InventoryResponse = serde_json::from_value(response.body)
            .map_err(|e| CapabilityError::ParseError(e.to_string()))?;

        info!("📦 Fetched inventory for folder {}: {} folders, {} items", 
              folder_id, 
              inventory.folders.len(), 
              inventory.items.len());

        Ok(inventory)
    }

    /// Get texture using the GetTexture capability
    pub async fn get_texture(&self, texture_id: Uuid) -> Result<Vec<u8>, CapabilityError> {
        let url_suffix = format!("/{}", texture_id);
        
        // For texture requests, we typically append the texture ID to the capability URL
        let capability = self.get_capability(well_known_capabilities::GET_TEXTURE).await
            .ok_or_else(|| CapabilityError::CapabilityNotFound(well_known_capabilities::GET_TEXTURE.to_string()))?;

        let texture_url = format!("{}{}", capability.url, url_suffix);
        
        debug!("🖼️ Requesting texture: {}", texture_id);
        
        let client = self.http_client.clone();
        
        let response = client.get(&texture_url)
            .header("User-Agent", "slv-rust/0.3.0")
            .send()
            .await
            .map_err(|e| CapabilityError::HttpError(e.to_string()))?;
            
        let status = response.status().as_u16();
        let texture_data = response.bytes().await
            .map_err(|e| CapabilityError::HttpError(e.to_string()))?
            .to_vec();

        if status >= 200 && status < 300 {
            info!("🖼️ Downloaded texture {}: {} bytes", texture_id, texture_data.len());
            Ok(texture_data)
        } else {
            warn!("❌ Failed to download texture {}: HTTP {}", texture_id, status);
            Err(CapabilityError::HttpError(format!("HTTP {}", status)))
        }
    }

    /// Get mesh using the GetMesh2 capability
    pub async fn get_mesh(&self, mesh_id: Uuid) -> Result<Vec<u8>, CapabilityError> {
        let url_suffix = format!("/{}", mesh_id);
        
        let capability = self.get_capability(well_known_capabilities::GET_MESH).await
            .ok_or_else(|| CapabilityError::CapabilityNotFound(well_known_capabilities::GET_MESH.to_string()))?;

        let mesh_url = format!("{}{}", capability.url, url_suffix);
        
        debug!("🔺 Requesting mesh: {}", mesh_id);
        
        let client = self.http_client.clone();
        
        let response = client.get(&mesh_url)
            .header("User-Agent", "slv-rust/0.3.0")
            .send()
            .await
            .map_err(|e| CapabilityError::HttpError(e.to_string()))?;
            
        let status = response.status().as_u16();
        let mesh_data = response.bytes().await
            .map_err(|e| CapabilityError::HttpError(e.to_string()))?
            .to_vec();

        if status >= 200 && status < 300 {
            info!("🔺 Downloaded mesh {}: {} bytes", mesh_id, mesh_data.len());
            Ok(mesh_data)
        } else {
            warn!("❌ Failed to download mesh {}: HTTP {}", mesh_id, status);
            Err(CapabilityError::HttpError(format!("HTTP {}", status)))
        }
    }

    /// Get display names using the GetDisplayNames capability
    pub async fn get_display_names(&self, agent_ids: Vec<Uuid>) -> Result<DisplayNamesResponse, CapabilityError> {
        let request_body = json!({
            "ids": agent_ids
        });

        let response = self.capability_request(
            well_known_capabilities::GET_DISPLAY_NAMES,
            CapabilityMethod::Post,
            Some(request_body),
        ).await?;

        let display_names: DisplayNamesResponse = serde_json::from_value(response.body)
            .map_err(|e| CapabilityError::ParseError(e.to_string()))?;

        info!("👤 Retrieved display names for {} agents", display_names.display_names.len());

        Ok(display_names)
    }

    /// Poll the event queue using EventQueueGet capability
    pub async fn poll_event_queue(&self, ack_id: Option<i64>) -> Result<EventQueueResponse, CapabilityError> {
        let mut request_body = json!({});
        
        if let Some(ack) = ack_id {
            request_body["ack"] = json!(ack);
        }

        let response = self.capability_request(
            well_known_capabilities::EVENT_QUEUE_GET,
            CapabilityMethod::Post,
            Some(request_body),
        ).await?;

        let event_queue: EventQueueResponse = serde_json::from_value(response.body)
            .map_err(|e| CapabilityError::ParseError(e.to_string()))?;

        if !event_queue.events.is_empty() {
            info!("📨 Received {} events from event queue", event_queue.events.len());
        }

        Ok(event_queue)
    }

    /// Get economy information
    pub async fn get_economy_info(&self) -> Result<EconomyResponse, CapabilityError> {
        let response = self.capability_request(
            well_known_capabilities::ECONOMY,
            CapabilityMethod::Get,
            None,
        ).await?;

        let economy: EconomyResponse = serde_json::from_value(response.body)
            .map_err(|e| CapabilityError::ParseError(e.to_string()))?;

        info!("💰 Retrieved economy information");

        Ok(economy)
    }
}

/// Response structure for inventory requests
#[derive(Debug, Deserialize, Serialize)]
pub struct InventoryResponse {
    pub folders: Vec<InventoryFolder>,
    pub items: Vec<InventoryItem>,
    pub version: i32,
    pub descendents: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InventoryFolder {
    pub folder_id: Uuid,
    pub parent_id: Uuid,
    pub name: String,
    pub type_default: i32,
    pub version: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InventoryItem {
    pub item_id: Uuid,
    pub parent_id: Uuid,
    pub asset_id: Uuid,
    pub name: String,
    pub description: String,
    pub asset_type: i32,
    pub inv_type: i32,
    pub permissions: InventoryPermissions,
    pub sale_info: SaleInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InventoryPermissions {
    pub base_mask: u32,
    pub owner_mask: u32,
    pub group_mask: u32,
    pub everyone_mask: u32,
    pub next_owner_mask: u32,
    pub creator_id: Uuid,
    pub owner_id: Uuid,
    pub last_owner_id: Uuid,
    pub group_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SaleInfo {
    pub sale_type: i32,
    pub sale_price: i32,
}

/// Response structure for display names requests
#[derive(Debug, Deserialize, Serialize)]
pub struct DisplayNamesResponse {
    pub display_names: Vec<DisplayName>,
    pub bad_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DisplayName {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub legacy_first_name: String,
    pub legacy_last_name: String,
    pub is_display_name_default: bool,
    pub display_name_expires: String,
    pub display_name_next_update: String,
}

/// Response structure for event queue requests
#[derive(Debug, Deserialize, Serialize)]
pub struct EventQueueResponse {
    pub events: Vec<EventQueueEvent>,
    pub id: i64,
    pub done: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventQueueEvent {
    pub message: String,
    pub body: serde_json::Value,
}

/// Response structure for economy requests
#[derive(Debug, Deserialize, Serialize)]
pub struct EconomyResponse {
    pub object_capacity: i32,
    pub object_count: i32,
    pub price_energy_unit: i32,
    pub price_object_claim: i32,
    pub price_public_object_decay: i32,
    pub price_public_object_delete: i32,
    pub price_parcel_claim: i32,
    pub price_parcel_rent: f32,
    pub price_upload: i32,
    pub price_rent_light: i32,
    pub teleport_min_price: i32,
    pub teleport_price_exponent: f32,
}