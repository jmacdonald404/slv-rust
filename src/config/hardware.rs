use crate::config::settings::{HardwareInfo, PerformanceProfile, PerformanceSettings};
use std::sync::Arc;
use sysinfo::System;

/// Detect hardware capabilities and recommend appropriate performance profile
pub async fn detect_hardware() -> anyhow::Result<HardwareInfo> {
    // Create wgpu instance for GPU detection
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        dx12_shader_compiler: wgpu::Dx12Compiler::default(),
        gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
    });

    // Request adapter to get GPU info
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

    let adapter_info = adapter.get_info();

    // Get system information
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_all();

    // Estimate GPU memory (this is approximate)
    let graphics_memory_mb = estimate_gpu_memory(&adapter_info);
    
    // Get total system memory
    let total_memory_gb = (system.total_memory() / (1024 * 1024 * 1024)) as u32;
    
    // Get CPU core count
    let cpu_cores = system.cpus().len() as u32;

    // Determine if GPU is integrated
    let is_integrated_gpu = is_integrated_graphics(&adapter_info);

    Ok(HardwareInfo {
        gpu_name: adapter_info.name.clone(),
        gpu_vendor: format!("{:?}", adapter_info.vendor),
        total_memory_gb,
        graphics_memory_mb,
        cpu_cores,
        is_integrated_gpu,
    })
}

/// Recommend performance profile based on hardware capabilities
pub fn recommend_profile(hardware: &HardwareInfo) -> PerformanceProfile {
    // Low-end hardware detection
    if hardware.total_memory_gb < 4 
        || hardware.is_integrated_gpu 
        || hardware.cpu_cores < 4 
        || hardware.graphics_memory_mb < 512 {
        return PerformanceProfile::Low;
    }

    // High-end hardware detection
    if hardware.total_memory_gb >= 16 
        && !hardware.is_integrated_gpu 
        && hardware.cpu_cores >= 8 
        && hardware.graphics_memory_mb >= 4096 {
        return PerformanceProfile::High;
    }

    // Default to balanced for mid-range hardware
    PerformanceProfile::Balanced
}

/// Initialize performance settings with hardware detection
pub async fn initialize_performance_settings() -> anyhow::Result<PerformanceSettings> {
    // Try to load existing settings first
    if let Some(settings) = crate::config::settings::load_performance_settings() {
        tracing::info!("Loaded existing performance settings: {:?}", settings.profile);
        return Ok(settings);
    }

    // No existing settings, perform hardware detection
    tracing::info!("No existing performance settings found, detecting hardware...");
    let hardware = detect_hardware().await?;
    
    tracing::info!("Hardware detected:");
    tracing::info!("  GPU: {} ({})", hardware.gpu_name, hardware.gpu_vendor);
    tracing::info!("  GPU Memory: {}MB", hardware.graphics_memory_mb);
    tracing::info!("  System Memory: {}GB", hardware.total_memory_gb);
    tracing::info!("  CPU Cores: {}", hardware.cpu_cores);
    tracing::info!("  Integrated GPU: {}", hardware.is_integrated_gpu);

    let recommended_profile = recommend_profile(&hardware);
    tracing::info!("Recommended performance profile: {:?}", recommended_profile);

    let settings = PerformanceSettings::for_profile(recommended_profile, &hardware);
    
    // Save the automatically detected settings
    if let Err(e) = crate::config::settings::save_performance_settings(&settings) {
        tracing::warn!("Failed to save performance settings: {}", e);
    } else {
        tracing::info!("Saved performance settings to disk");
    }

    Ok(settings)
}

/// Estimate GPU memory based on adapter info
fn estimate_gpu_memory(adapter_info: &wgpu::AdapterInfo) -> u32 {
    // This is a rough estimation based on GPU type and vendor
    // In a real application, you might use platform-specific APIs
    // to get more accurate memory information
    
    match adapter_info.device_type {
        wgpu::DeviceType::IntegratedGpu => {
            // Integrated GPUs typically share system memory
            // Estimate based on common configurations
            match adapter_info.vendor {
                0x8086 => 512,  // Intel
                0x1002 => 1024, // AMD
                _ => 512,
            }
        }
        wgpu::DeviceType::DiscreteGpu => {
            // Discrete GPUs - estimate based on vendor and name patterns
            let name_lower = adapter_info.name.to_lowercase();
            
            if name_lower.contains("rtx 4090") || name_lower.contains("rtx 4080") {
                16384 // 16GB
            } else if name_lower.contains("rtx 4070") || name_lower.contains("rtx 3080") {
                12288 // 12GB
            } else if name_lower.contains("rtx 3070") || name_lower.contains("rtx 4060") {
                8192 // 8GB
            } else if name_lower.contains("rtx 3060") || name_lower.contains("gtx 1660") {
                6144 // 6GB
            } else if name_lower.contains("rx 7900") || name_lower.contains("rx 6900") {
                16384 // 16GB
            } else if name_lower.contains("rx 6800") || name_lower.contains("rx 7800") {
                12288 // 12GB
            } else if name_lower.contains("rx 6700") || name_lower.contains("rx 7700") {
                8192 // 8GB
            } else {
                // Default conservative estimate for unknown discrete GPUs
                4096 // 4GB
            }
        }
        wgpu::DeviceType::VirtualGpu => 2048,  // 2GB
        wgpu::DeviceType::Cpu => 0,            // Software rendering
        wgpu::DeviceType::Other => 1024,       // 1GB default
    }
}

/// Determine if the GPU is integrated graphics
fn is_integrated_graphics(adapter_info: &wgpu::AdapterInfo) -> bool {
    match adapter_info.device_type {
        wgpu::DeviceType::IntegratedGpu => true,
        wgpu::DeviceType::DiscreteGpu => false,
        _ => {
            // For other types, check the name for common integrated GPU patterns
            let name_lower = adapter_info.name.to_lowercase();
            name_lower.contains("intel hd") 
                || name_lower.contains("intel iris") 
                || name_lower.contains("intel uhd")
                || name_lower.contains("amd radeon(tm) graphics")
                || name_lower.contains("apple m1")
                || name_lower.contains("apple m2")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hardware_detection() {
        let result = detect_hardware().await;
        assert!(result.is_ok());
        
        let hardware = result.unwrap();
        assert!(!hardware.gpu_name.is_empty());
        assert!(hardware.total_memory_gb > 0);
        assert!(hardware.cpu_cores > 0);
    }

    #[test]
    fn test_profile_recommendation() {
        // Test low-end hardware
        let low_end = HardwareInfo {
            gpu_name: "Intel HD Graphics".to_string(),
            gpu_vendor: "Intel".to_string(),
            total_memory_gb: 2,
            graphics_memory_mb: 256,
            cpu_cores: 2,
            is_integrated_gpu: true,
        };
        assert_eq!(recommend_profile(&low_end), PerformanceProfile::Low);

        // Test high-end hardware
        let high_end = HardwareInfo {
            gpu_name: "RTX 4090".to_string(),
            gpu_vendor: "NVIDIA".to_string(),
            total_memory_gb: 32,
            graphics_memory_mb: 16384,
            cpu_cores: 16,
            is_integrated_gpu: false,
        };
        assert_eq!(recommend_profile(&high_end), PerformanceProfile::High);

        // Test balanced hardware
        let balanced = HardwareInfo {
            gpu_name: "RTX 3060".to_string(),
            gpu_vendor: "NVIDIA".to_string(),
            total_memory_gb: 8,
            graphics_memory_mb: 6144,
            cpu_cores: 6,
            is_integrated_gpu: false,
        };
        assert_eq!(recommend_profile(&balanced), PerformanceProfile::Balanced);
    }
}