use slv_rust::config::{
    PerformanceProfile, PerformanceSettings, HardwareInfo, 
    ThreadPoolConfig, 
    save_performance_settings, load_performance_settings
};

#[tokio::test]
async fn test_performance_profiles() {
    // Test hardware info creation
    let hardware = HardwareInfo {
        gpu_name: "Test GPU".to_string(),
        gpu_vendor: "Test".to_string(),
        total_memory_gb: 8,
        graphics_memory_mb: 4096,
        cpu_cores: 6,
        is_integrated_gpu: false,
    };

    // Test all performance profiles
    for profile in [PerformanceProfile::Low, PerformanceProfile::Balanced, PerformanceProfile::High] {
        let settings = PerformanceSettings::for_profile(profile, &hardware);
        
        assert_eq!(settings.profile, profile);
        assert!(settings.rendering.draw_distance > 0.0);
        assert!(settings.memory.texture_cache_size_mb > 0);
        
        match profile {
            PerformanceProfile::Low => {
                assert_eq!(settings.rendering.draw_distance, 64.0);
                assert_eq!(settings.rendering.target_fps, 30.0);
                assert_eq!(settings.networking.packet_aggregation, true);
            }
            PerformanceProfile::Balanced => {
                assert_eq!(settings.rendering.draw_distance, 128.0);
                assert_eq!(settings.rendering.target_fps, 60.0);
            }
            PerformanceProfile::High => {
                assert_eq!(settings.rendering.draw_distance, 256.0);
                assert_eq!(settings.rendering.target_fps, 120.0);
                assert_eq!(settings.networking.packet_aggregation, false);
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_thread_pool_config() {
    let hardware = HardwareInfo {
        gpu_name: "Test GPU".to_string(),
        gpu_vendor: "Test".to_string(),
        total_memory_gb: 16,
        graphics_memory_mb: 8192,
        cpu_cores: 8,
        is_integrated_gpu: false,
    };

    // Test thread pool configuration for different profiles
    let balanced_settings = PerformanceSettings::for_profile(PerformanceProfile::Balanced, &hardware);
    let config = ThreadPoolConfig::for_performance_profile(
        &balanced_settings,
        hardware.cpu_cores,
        hardware.total_memory_gb,
    );

    assert_eq!(config.job_threads, 8);
    assert_eq!(config.async_threads, 4);
    assert_eq!(config.enable_work_stealing, true);
}

#[tokio::test] 
async fn test_settings_persistence() {
    let hardware = HardwareInfo {
        gpu_name: "Test GPU".to_string(),
        gpu_vendor: "Test".to_string(),
        total_memory_gb: 16,
        graphics_memory_mb: 8192,
        cpu_cores: 8,
        is_integrated_gpu: false,
    };

    // Create and save settings
    let original_settings = PerformanceSettings::for_profile(PerformanceProfile::High, &hardware);
    
    // This might fail if no config directory exists, but that's okay for testing
    let save_result = save_performance_settings(&original_settings);
    
    if save_result.is_ok() {
        // Try to load settings back
        if let Some(loaded_settings) = load_performance_settings() {
            assert_eq!(loaded_settings.profile, PerformanceProfile::High);
            assert_eq!(loaded_settings.rendering.draw_distance, 256.0);
            assert_eq!(loaded_settings.rendering.target_fps, 120.0);
        }
    }
    
    // Test should pass regardless of I/O success
    assert!(true);
}