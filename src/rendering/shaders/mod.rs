// Shader system with quality variants and pipeline caching
pub mod quality_variants;

pub use quality_variants::{ShaderGenerator, ShaderCache, ShaderKey, ShaderType};