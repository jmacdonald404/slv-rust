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

    // Additional fields from login response that were missing
    pub inventory_lib_root: Option<Vec<InventoryFolder>>,
    pub inventory_lib_owner: Option<Vec<LibraryOwner>>,
    pub inventory_skel_lib: Option<Vec<InventoryItem>>,
    pub initial_outfit: Option<Vec<OutfitItem>>,
    pub gestures: Option<Vec<Gesture>>,
    pub display_names: Option<DisplayNameConfig>,
    pub event_categories: Option<Vec<EventCategory>>,
    pub event_notifications: Option<Vec<EventNotification>>,
    pub classified_categories: Option<Vec<ClassifiedCategory>>,
    pub newuser_config: Option<NewUserConfig>,
    pub ui_config: Option<UiConfig>,
    pub voice_config: Option<VoiceConfig>,
    pub tutorial_setting: Option<TutorialConfig>,
    pub global_textures: Option<Vec<GlobalTexture>>,
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

/// Library owner information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryOwner {
    pub agent_id: Option<String>,
}

/// Inventory item information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub item_id: Option<String>,
    pub name: Option<String>,
    pub desc: Option<String>,
    pub asset_id: Option<String>,
    pub inv_type: Option<u32>,
    pub type_: Option<u32>,
    pub flags: Option<u32>,
    pub sale_type: Option<u32>,
    pub sale_price: Option<u32>,
    pub permissions: Option<ItemPermissions>,
}

/// Item permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPermissions {
    pub base_mask: Option<u32>,
    pub owner_mask: Option<u32>,
    pub group_mask: Option<u32>,
    pub everyone_mask: Option<u32>,
    pub next_owner_mask: Option<u32>,
    pub creator_id: Option<String>,
    pub owner_id: Option<String>,
    pub last_owner_id: Option<String>,
    pub group_id: Option<String>,
}

/// Outfit item information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutfitItem {
    pub folder_name: Option<String>,
    pub gender: Option<String>,
}

/// Gesture information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gesture {
    pub item_id: Option<String>,
    pub asset_id: Option<String>,
}

/// Display name configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayNameConfig {
    pub display_name_enabled: Option<bool>,
    pub display_name_update_max_time: Option<u32>,
}

/// Event category information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCategory {
    pub category_id: Option<u32>,
    pub category_name: Option<String>,
}

/// Event notification information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotification {
    pub event_id: Option<u32>,
    pub event_name: Option<String>,
}

/// Classified category information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedCategory {
    pub category_id: Option<u32>,
    pub category_name: Option<String>,
}

/// New user configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserConfig {
    pub max_spins: Option<u32>,
    pub tutorial_url: Option<String>,
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub allow_first_life: Option<bool>,
    pub allow_mature_publish: Option<bool>,
}

/// Voice configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    pub voice_server_type: Option<String>,
}

/// Tutorial configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialConfig {
    pub tutorial_url: Option<String>,
}

/// Global texture information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTexture {
    pub sun_texture_id: Option<String>,
    pub moon_texture_id: Option<String>,
    pub cloud_texture_id: Option<String>,
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