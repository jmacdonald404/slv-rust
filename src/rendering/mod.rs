pub mod camera;
pub mod camera_uniform;
pub mod engine;
pub mod materials;
pub mod scene;
pub mod shaders;
pub mod light;
pub mod performance_renderer;

// Re-export the performance renderer as the main interface
pub use performance_renderer::{PerformanceRenderer, PerformanceMetrics};