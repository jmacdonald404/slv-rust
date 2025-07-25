pub mod settings;
pub mod hardware;
pub mod concurrency;

// Re-export commonly used types
pub use settings::{
    PerformanceProfile, PerformanceSettings, PerformanceSettingsHandle,
    RenderingSettings, MemorySettings, NetworkingSettings, HardwareInfo,
    TextureQuality, ShaderQuality, ShadowQuality, StreamingPriority,
    create_performance_settings_handle, save_performance_settings, load_performance_settings,
};
pub use hardware::{detect_hardware, recommend_profile, initialize_performance_settings};
pub use concurrency::{
    ConcurrencyManager, ThreadPoolConfig, ConcurrencyStats,
    initialize_concurrency, get_concurrency_manager, dod_utils,
};
