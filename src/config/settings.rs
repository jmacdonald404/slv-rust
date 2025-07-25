use crate::ui::PreferencesState;
use crate::ui::proxy::ProxySettings;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use directories::ProjectDirs;
use toml;
use serde::{Serialize, Deserialize};

const CONFIG_FILE: &str = "preferences.toml";
const PERFORMANCE_CONFIG_FILE: &str = "performance.toml";

// =============================================================================
// Performance Configuration System
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceProfile {
    Low,      // Optimized for low-end hardware
    Balanced, // Default for mid-range systems
    High,     // Maximum quality for high-end hardware
    Custom,   // User-defined granular controls
}

impl Default for PerformanceProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureQuality {
    Low,
    Medium,
    High,
    Ultra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderQuality {
    Simplified,
    Standard,
    Enhanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowQuality {
    Off,
    Low,
    Medium,
    High,
    Ultra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamingPriority {
    Memory,   // Prioritize memory usage
    Balanced, // Balance memory and quality
    Quality,  // Prioritize visual quality
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderingSettings {
    pub draw_distance: f32,
    pub texture_quality: TextureQuality,
    pub shader_quality: ShaderQuality,
    pub shadow_quality: ShadowQuality,
    pub hzb_enabled: bool,
    pub cluster_resolution: (usize, usize, usize), // (x, y, z) cluster dimensions
    pub vsync_enabled: bool,
    pub target_fps: f32,
}

impl Default for RenderingSettings {
    fn default() -> Self {
        Self {
            draw_distance: 128.0,
            texture_quality: TextureQuality::Medium,
            shader_quality: ShaderQuality::Standard,
            shadow_quality: ShadowQuality::Medium,
            hzb_enabled: true,
            cluster_resolution: (16, 8, 16),
            vsync_enabled: true,
            target_fps: 60.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySettings {
    pub texture_cache_size_mb: u32,
    pub mesh_cache_size_mb: u32,
    pub asset_cache_size_mb: u32,
    pub mesh_simplification_aggressive: bool,
    pub impostor_distance_multiplier: f32,
    pub streaming_priority: StreamingPriority,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            texture_cache_size_mb: 512,
            mesh_cache_size_mb: 256,
            asset_cache_size_mb: 1024,
            mesh_simplification_aggressive: false,
            impostor_distance_multiplier: 1.0,
            streaming_priority: StreamingPriority::Balanced,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkingSettings {
    pub packet_aggregation: bool,
    pub compression_level: u8, // 0-9, where 0 is no compression
    pub bandwidth_limit_kbps: Option<u32>,
    pub connection_timeout_ms: u32,
    pub retry_attempts: u32,
    pub enable_proxy: bool,
}

impl Default for NetworkingSettings {
    fn default() -> Self {
        Self {
            packet_aggregation: true,
            compression_level: 3,
            bandwidth_limit_kbps: Some(1024),
            connection_timeout_ms: 5000,
            retry_attempts: 3,
            enable_proxy: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    pub profile: PerformanceProfile,
    pub rendering: RenderingSettings,
    pub memory: MemorySettings,
    pub networking: NetworkingSettings,
    pub adaptive_scaling_enabled: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            profile: PerformanceProfile::default(),
            rendering: RenderingSettings::default(),
            memory: MemorySettings::default(),
            networking: NetworkingSettings::default(),
            adaptive_scaling_enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub gpu_name: String,
    pub gpu_vendor: String,
    pub total_memory_gb: u32,
    pub graphics_memory_mb: u32,
    pub cpu_cores: u32,
    pub is_integrated_gpu: bool,
}

impl PerformanceSettings {
    pub fn for_profile(profile: PerformanceProfile, hardware: &HardwareInfo) -> Self {
        let (rendering, memory, networking) = match profile {
            PerformanceProfile::Low => (
                RenderingSettings {
                    draw_distance: 64.0,
                    texture_quality: TextureQuality::Low,
                    shader_quality: ShaderQuality::Simplified,
                    shadow_quality: ShadowQuality::Off,
                    hzb_enabled: false,
                    cluster_resolution: (8, 4, 8),
                    vsync_enabled: true,
                    target_fps: 30.0,
                },
                MemorySettings {
                    texture_cache_size_mb: (hardware.graphics_memory_mb / 4).min(256),
                    mesh_cache_size_mb: (hardware.total_memory_gb * 64).min(256),
                    asset_cache_size_mb: (hardware.total_memory_gb * 128).min(512),
                    mesh_simplification_aggressive: true,
                    impostor_distance_multiplier: 0.5,
                    streaming_priority: StreamingPriority::Memory,
                },
                NetworkingSettings {
                    packet_aggregation: true,
                    compression_level: 6,
                    bandwidth_limit_kbps: Some(256),
                    connection_timeout_ms: 10000,
                    retry_attempts: 5,
                    enable_proxy: false,
                },
            ),
            PerformanceProfile::Balanced => (
                RenderingSettings {
                    draw_distance: 128.0,
                    texture_quality: TextureQuality::Medium,
                    shader_quality: ShaderQuality::Standard,
                    shadow_quality: ShadowQuality::Medium,
                    hzb_enabled: true,
                    cluster_resolution: (16, 8, 16),
                    vsync_enabled: true,
                    target_fps: 60.0,
                },
                MemorySettings {
                    texture_cache_size_mb: (hardware.graphics_memory_mb / 2).min(1024),
                    mesh_cache_size_mb: (hardware.total_memory_gb * 128).min(1024),
                    asset_cache_size_mb: (hardware.total_memory_gb * 256).min(2048),
                    mesh_simplification_aggressive: false,
                    impostor_distance_multiplier: 1.0,
                    streaming_priority: StreamingPriority::Balanced,
                },
                NetworkingSettings {
                    packet_aggregation: true,
                    compression_level: 3,
                    bandwidth_limit_kbps: Some(1024),
                    connection_timeout_ms: 5000,
                    retry_attempts: 3,
                    enable_proxy: false,
                },
            ),
            PerformanceProfile::High => (
                RenderingSettings {
                    draw_distance: 256.0,
                    texture_quality: TextureQuality::High,
                    shader_quality: ShaderQuality::Enhanced,
                    shadow_quality: ShadowQuality::High,
                    hzb_enabled: true,
                    cluster_resolution: (32, 16, 32),
                    vsync_enabled: false, // Allow higher FPS
                    target_fps: 120.0,
                },
                MemorySettings {
                    texture_cache_size_mb: (hardware.graphics_memory_mb * 3 / 4).min(4096),
                    mesh_cache_size_mb: (hardware.total_memory_gb * 256).min(4096),
                    asset_cache_size_mb: (hardware.total_memory_gb * 512).min(8192),
                    mesh_simplification_aggressive: false,
                    impostor_distance_multiplier: 2.0,
                    streaming_priority: StreamingPriority::Quality,
                },
                NetworkingSettings {
                    packet_aggregation: false,
                    compression_level: 1,
                    bandwidth_limit_kbps: None,
                    connection_timeout_ms: 2000,
                    retry_attempts: 2,
                    enable_proxy: false,
                },
            ),
            PerformanceProfile::Custom => {
                // For custom, return defaults that user can modify
                (RenderingSettings::default(), MemorySettings::default(), NetworkingSettings::default())
            }
        };

        Self {
            profile,
            rendering,
            memory,
            networking,
            adaptive_scaling_enabled: profile != PerformanceProfile::Custom,
        }
    }
}

pub type PerformanceSettingsHandle = Arc<RwLock<PerformanceSettings>>;

pub fn create_performance_settings_handle(settings: PerformanceSettings) -> PerformanceSettingsHandle {
    Arc::new(RwLock::new(settings))
}

// Performance configuration file management
fn performance_config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "slv", "slv-rust")
        .map(|proj| proj.config_dir().join(PERFORMANCE_CONFIG_FILE))
}

pub fn save_performance_settings(settings: &PerformanceSettings) -> std::io::Result<()> {
    if let Some(path) = performance_config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml = toml::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, toml)?;
    }
    Ok(())
}

pub fn load_performance_settings() -> Option<PerformanceSettings> {
    if let Some(path) = performance_config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(settings) = toml::from_str::<PerformanceSettings>(&data) {
                return Some(settings);
            }
        }
    }
    None
}

#[derive(Serialize, Deserialize)]
pub struct PreferencesToml {
    pub enable_sound: bool,
    pub volume: f32,
    pub graphics_api: String,
    pub vsync: bool,
    pub render_distance: u32,
    pub max_bandwidth: u32,
    pub timeout: u32,
}

impl From<&PreferencesState> for PreferencesToml {
    fn from(p: &PreferencesState) -> Self {
        Self {
            enable_sound: p.enable_sound,
            volume: p.volume,
            graphics_api: p.graphics_api.clone(),
            vsync: p.vsync,
            render_distance: p.render_distance,
            max_bandwidth: p.max_bandwidth,
            timeout: p.timeout,
        }
    }
}

impl Into<PreferencesState> for PreferencesToml {
    fn into(self) -> PreferencesState {
        PreferencesState {
            enable_sound: self.enable_sound,
            volume: self.volume,
            graphics_api: self.graphics_api,
            vsync: self.vsync,
            render_distance: self.render_distance,
            max_bandwidth: self.max_bandwidth,
            timeout: self.timeout,
            udp_test_result: None,
            udp_test_in_progress: false,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "slv", "slv-rust")
        .map(|proj| proj.config_dir().join(CONFIG_FILE))
}

pub fn save_preferences(prefs: &PreferencesState) -> std::io::Result<()> {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml = toml::to_string_pretty(&PreferencesToml::from(prefs)).unwrap();
        fs::write(path, toml)?;
    }
    Ok(())
}

pub fn load_preferences() -> Option<PreferencesState> {
    if let Some(path) = config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(toml) = toml::from_str::<PreferencesToml>(&data) {
                return Some(toml.into());
            }
        }
    }
    None
}

const GENERAL_CONFIG_FILE: &str = "general_settings.toml";

#[derive(Serialize, Deserialize)]
pub struct GeneralSettingsToml {
    pub preferences: PreferencesToml,
    pub proxy: ProxySettings,
}

impl GeneralSettingsToml {
    pub fn from_states(prefs: &PreferencesState, proxy: &ProxySettings) -> Self {
        Self {
            preferences: PreferencesToml::from(prefs),
            proxy: proxy.clone(),
        }
    }
    pub fn into_states(self) -> (PreferencesState, ProxySettings) {
        (self.preferences.into(), self.proxy)
    }
}

fn general_config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "slv", "slv-rust")
        .map(|proj| proj.config_dir().join(GENERAL_CONFIG_FILE))
}

pub fn save_general_settings(prefs: &PreferencesState, proxy: &ProxySettings) -> std::io::Result<()> {
    if let Some(path) = general_config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml = toml::to_string_pretty(&GeneralSettingsToml::from_states(prefs, proxy)).unwrap();
        fs::write(path, toml)?;
    }
    Ok(())
}

pub fn load_general_settings() -> Option<(PreferencesState, ProxySettings)> {
    if let Some(path) = general_config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(toml) = toml::from_str::<GeneralSettingsToml>(&data) {
                return Some(toml.into_states());
            }
        }
    }
    None
}
