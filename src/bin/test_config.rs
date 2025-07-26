
use slv_rust::config::*;
use slv_rust::utils::logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    logging::init_logging();
    
    println!("Testing Performance Configuration System");
    println!("========================================");
    
    // Test hardware detection and settings initialization
    match initialize_performance_settings().await {
        Ok(settings) => {
            println!("✅ Hardware detection successful!");
            println!("   Profile: {:?}", settings.profile);
            println!("   Draw Distance: {}", settings.rendering.draw_distance);
            println!("   Texture Cache: {}MB", settings.memory.texture_cache_size_mb);
            println!("   Bandwidth Limit: {:?}", settings.networking.bandwidth_limit_kbps);
        }
        Err(e) => {
            println!("❌ Hardware detection failed: {}", e);
            println!("   Falling back to default settings...");
        }
    }
    
    // Test profile switching
    println!("\nTesting Profile Switching");
    println!("=========================");
    
    let mock_hardware = HardwareInfo {
        gpu_name: "Test GPU".to_string(),
        gpu_vendor: "Test".to_string(),
        total_memory_gb: 16,
        graphics_memory_mb: 8192,
        cpu_cores: 8,
        is_integrated_gpu: false,
    };
    
    for profile in [PerformanceProfile::Low, PerformanceProfile::Balanced, PerformanceProfile::High] {
        let settings = PerformanceSettings::for_profile(profile, &mock_hardware);
        println!("Profile {:?}:", profile);
        println!("  Draw Distance: {}", settings.rendering.draw_distance);
        println!("  Texture Quality: {:?}", settings.rendering.texture_quality);
        println!("  Target FPS: {}", settings.rendering.target_fps);
        println!("  Texture Cache: {}MB", settings.memory.texture_cache_size_mb);
        println!("  Packet Aggregation: {}", settings.networking.packet_aggregation);
        println!();
    }
    
    // Test concurrency system
    println!("Testing Concurrency System");
    println!("==========================");
    
    let thread_config = ThreadPoolConfig::for_performance_profile(
        &PerformanceSettings::for_profile(PerformanceProfile::Balanced, &mock_hardware),
        mock_hardware.cpu_cores,
        mock_hardware.total_memory_gb,
    );
    
    println!("Thread Pool Config:");
    println!("  Job threads: {}", thread_config.job_threads);
    println!("  Async threads: {}", thread_config.async_threads);
    println!("  Work stealing: {}", thread_config.enable_work_stealing);
    
    match initialize_concurrency(thread_config) {
        Ok(()) => {
            println!("✅ Concurrency system initialized successfully!");
            
            // Test job execution
            if let Some(manager) = get_concurrency_manager() {
                let result = manager.execute_job(|| {
                    (1..1000).sum::<i32>()
                });
                println!("  Job execution test result: {}", result);
                
                // Test parallel processing
                let data = vec![1, 2, 3, 4, 5];
                let results = manager.execute_parallel(&data, |&x| x * x);
                println!("  Parallel processing test: {:?}", results);
                
                // Test async execution
                let handle = manager.spawn_async(async {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    "async task completed"
                });
                
                match handle.await {
                    Ok(result) => println!("  Async execution test: {}", result),
                    Err(e) => println!("  Async execution failed: {}", e),
                }
            }
        }
        Err(e) => {
            println!("❌ Concurrency system initialization failed: {}", e);
        }
    }
    
    // Test configuration persistence
    println!("\nTesting Configuration Persistence");
    println!("=================================");
    
    let test_settings = PerformanceSettings::for_profile(PerformanceProfile::High, &mock_hardware);
    match save_performance_settings(&test_settings) {
        Ok(()) => println!("✅ Settings saved successfully!"),
        Err(e) => println!("❌ Settings save failed: {}", e),
    }
    
    match load_performance_settings() {
        Some(loaded_settings) => {
            println!("✅ Settings loaded successfully!");
            println!("   Loaded profile: {:?}", loaded_settings.profile);
        }
        None => println!("❌ Settings load failed"),
    }
    
    println!("\n🎉 Configuration system test completed!");
    Ok(())
}