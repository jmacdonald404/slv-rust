use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::utils::math::{Vector3, RegionHandle};

/// Comprehensive login response structure matching Second Life's schema
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginResponse {
    // Core login fields
    pub success: bool,
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub secure_session_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub circuit_code: u32,
    pub simulator_ip: String,
    pub simulator_port: u16,
    pub look_at: Vector3,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub seed_capability: Option<String>,

    // Additional fields from Second Life
    pub agent_access: Option<String>,
    pub agent_access_max: Option<String>,
    pub agent_region_access: Option<String>,
    pub agent_appearance_service: Option<String>,
    pub agent_flags: Option<u32>,
    pub max_agent_groups: Option<u32>,
    pub openid_url: Option<String>,
    pub openid_token: Option<String>,
    pub cof_version: Option<u32>,
    pub account_type: Option<String>,
    pub linden_status_code: Option<String>,
    pub max_god_level: Option<u32>,
    pub god_level: Option<u32>,
    pub seconds_since_epoch: Option<u64>,
    pub start_location: Option<Vector3>,
    pub home: Option<Vector3>,
    pub region_x: Option<i32>,
    pub region_y: Option<i32>,

    // Home information
    pub home_info: Option<HomeInfo>,

    // Inventory data
    pub inventory_root: Option<Vec<InventoryFolder>>,
    pub inventory_skeleton: Option<Vec<InventoryFolder>>,

    // Buddy list
    pub buddy_list: Option<Vec<BuddyInfo>>,

    // Login flags
    pub login_flags: Option<Vec<LoginFlag>>,

    // Premium packages and benefits
    pub premium_packages: Option<std::collections::HashMap<String, PremiumPackage>>,
    pub account_level_benefits: Option<PremiumPackageBenefits>,

    // Network and server information
    pub map_server_url: Option<String>,
    pub udp_blacklist: Option<Vec<String>>,
}

/// Home location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeInfo {
    pub region_handle: RegionHandle,
    pub position: Vector3,
    pub look_at: Vector3,
}

/// Inventory folder information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryFolder {
    pub folder_id: Option<String>,
    pub name: Option<String>,
    pub parent_id: Option<String>,
    pub type_default: Option<u32>,
    pub version: Option<u32>,
}

/// Buddy list information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuddyInfo {
    pub buddy_id: Option<String>,
    pub buddy_rights_has: Option<u32>,
    pub buddy_rights_given: Option<u32>,
}

/// Login flag information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFlag {
    pub stipend_since_login: Option<String>,
    pub ever_logged_in: Option<String>,
    pub gendered: Option<String>,
    pub daylight_savings: Option<String>,
}

/// Premium package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumPackage {
    pub benefits: Option<PremiumPackageBenefits>,
    pub description: Option<PackageDescription>,
}

/// Premium package benefits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumPackageBenefits {
    pub animated_object_limit: Option<u32>,
    pub animation_upload_cost: Option<u32>,
    pub attachment_limit: Option<u32>,
    pub beta_grid_land: Option<u32>,
    pub create_group_cost: Option<u32>,
    pub create_repeating_events: Option<u32>,
    pub estate_access_token: Option<String>,
    pub gridwide_experience_limit: Option<u32>,
    pub group_membership_limit: Option<u32>,
    pub land_auctions_allowed: Option<u32>,
    pub lastname_change_allowed: Option<u32>,
    pub lastname_change_cost: Option<u32>,
    pub lastname_change_rate: Option<u32>,
    pub linden_buy_fee: Option<u32>,
    pub linden_homes: Option<Vec<String>>,
    pub live_chat: Option<u32>,
    pub local_experiences: Option<u32>,
    pub mainland_tier: Option<u32>,
    pub marketplace_concierge_support: Option<u32>,
    pub marketplace_listing_limit: Option<u32>,
    pub marketplace_ple_limit: Option<u32>,
    pub mesh_upload_cost: Option<u32>,
    pub object_account_level: Option<u32>,
    pub one_time_event_allowed: Option<u32>,
    pub one_time_event_cost: Option<u32>,
    pub partner_fee: Option<u32>,
    pub phone_support: Option<u32>,
    pub picks_limit: Option<u32>,
    pub place_pages: Option<PlacePages>,
    pub premium_access: Option<u32>,
    pub premium_alts: Option<u32>,
    pub premium_gifts: Option<u32>,
    pub priority_entry: Option<u32>,
    pub repeating_events_cost: Option<u32>,
    pub script_limit: Option<u32>,
    pub signup_bonus: Option<u32>,
    pub sound_upload_cost: Option<u32>,
    pub stipend: Option<u32>,
    pub stored_im_limit: Option<u32>,
    #[serde(default)]
    pub large_texture_upload_cost: Option<Vec<u32>>,
    #[serde(default)]
    pub texture_upload_cost: Option<u32>,
    pub transaction_history_limit: Option<u32>,
    pub unpartner_fee: Option<u32>,
    pub use_animesh: Option<u32>,
    pub voice_morphing: Option<u32>,
}

/// Place pages information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacePages {
    pub additional_listing_cost: Option<u32>,
    pub num_free_listings: Option<u32>,
}

/// Package description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDescription {
    pub name: Option<std::collections::HashMap<String, String>>,
}

impl LoginResponse {
    /// Get simulator address as SocketAddr
    pub fn simulator_address(&self) -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.simulator_ip, self.simulator_port);
        addr.parse().map_err(|e: std::net::AddrParseError| e.into())
    }

    /// Get full agent name
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    /// Check if login was successful
    pub fn is_successful(&self) -> bool {
        self.success
    }

    /// Get error message if login failed
    pub fn error_message(&self) -> Option<&str> {
        if !self.success {
            self.reason.as_deref().or(self.message.as_deref())
        } else {
            None
        }
    }
} 