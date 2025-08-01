// SLV-Rust: Second Life Viewer in Rust
// Performance by Default, Scalable by Design

#![allow(warnings)]

pub mod utils;
pub mod networking;
pub mod ui;
pub mod config;
pub mod app;

// Include modules that exist
pub mod rendering;
pub mod assets;
pub mod world;

// Re-export commonly used types for convenience
pub use config::{
    PerformanceProfile, PerformanceSettings, HardwareInfo,
    initialize_performance_settings, initialize_concurrency,
};

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
