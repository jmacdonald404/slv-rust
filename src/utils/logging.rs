use tracing::{Level, Subscriber};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};
use std::env;

/// Initialize logging with comprehensive configuration
pub fn init_logging() {
    // Check for environment variables to configure logging
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let enable_wgpu_logging = env::var("WGPU_LOG").unwrap_or_else(|_| "1".to_string()) == "1";
    let enable_backtrace = env::var("RUST_BACKTRACE").unwrap_or_else(|_| "0".to_string()) == "1";

    // Create environment filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            let mut filter = EnvFilter::new(&log_level);
            
            // Add specific filters for better visibility
            if enable_wgpu_logging {
                filter = filter.add_directive("wgpu=debug".parse().unwrap());
                filter = filter.add_directive("wgpu_core=debug".parse().unwrap());
                filter = filter.add_directive("wgpu_hal=debug".parse().unwrap());
                filter = filter.add_directive("naga=debug".parse().unwrap());
            }
            
            // Add filters for our application
            filter = filter.add_directive("slv_rust=debug".parse().unwrap());
            
            // Add filters for winit and other dependencies
            filter = filter.add_directive("winit=debug".parse().unwrap());
            filter = filter.add_directive("pollster=debug".parse().unwrap());
            
            filter
        });

    // Create the subscriber with multiple layers
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer()
            .with_span_events(FmtSpan::CLOSE)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(true)
        );

    // Initialize the subscriber
    subscriber.init();

    // Set up panic hook with better logging
    let enable_backtrace = enable_backtrace; // Clone for the closure
    std::panic::set_hook(Box::new(move |panic_info| {
        tracing::error!("Panic occurred: {}", panic_info);
        
        if let Some(location) = panic_info.location() {
            tracing::error!(
                "Panic location: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }

        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            tracing::error!("Panic payload: {}", s);
        }

        if enable_backtrace {
            tracing::error!("Backtrace:\n{:?}", std::backtrace::Backtrace::capture());
        }
    }));

    // Log startup information
    tracing::info!("Logging initialized with level: {}", log_level);
    tracing::info!("WGPU logging enabled: {}", enable_wgpu_logging);
    tracing::info!("Backtrace enabled: {}", enable_backtrace);
}

/// Log system information for debugging
pub fn log_system_info() {
    tracing::info!("=== System Information ===");
    tracing::info!("OS: {}", std::env::consts::OS);
    tracing::info!("Architecture: {}", std::env::consts::ARCH);
    tracing::info!("Rust Version: {}", env!("CARGO_PKG_VERSION"));
    
    // Log environment variables that affect graphics
    if let Ok(backend) = env::var("WGPU_BACKEND") {
        tracing::info!("WGPU Backend: {}", backend);
    }
    
    if let Ok(adapter) = env::var("WGPU_ADAPTER_NAME") {
        tracing::info!("WGPU Adapter: {}", adapter);
    }
    
    if let Ok(validation) = env::var("WGPU_VALIDATION") {
        tracing::info!("WGPU Validation: {}", validation);
    }
    
    tracing::info!("========================");
}

/// Create a custom error handler for wgpu operations
pub fn handle_wgpu_result<T, E: std::fmt::Display>(result: Result<T, E>, operation: &str) -> Result<T, E> {
    match &result {
        Ok(_) => {
            tracing::debug!("WGPU operation '{}' completed successfully", operation);
        }
        Err(e) => {
            tracing::error!("WGPU operation '{}' failed: {}", operation, e);
        }
    }
    result
}

/// Log wgpu adapter information
pub fn log_adapter_info(adapter: &wgpu::Adapter) {
    let info = adapter.get_info();
    tracing::info!("=== WGPU Adapter Information ===");
    tracing::info!("Name: {}", info.name);
    tracing::info!("Backend: {:?}", info.backend);
    tracing::info!("Device Type: {:?}", info.device_type);
    tracing::info!("Vendor: {}", info.vendor);
    tracing::info!("Device: {}", info.device);
    tracing::info!("Driver: {}", info.driver);
    tracing::info!("Driver Info: {}", info.driver_info);
    tracing::info!("=================================");
}

/// Log wgpu device limits and features
pub fn log_device_info(device: &wgpu::Device) {
    let limits = device.limits();
    let features = device.features();
    
    tracing::info!("=== WGPU Device Information ===");
    tracing::info!("Max Texture Dimension 1D: {}", limits.max_texture_dimension_1d);
    tracing::info!("Max Texture Dimension 2D: {}", limits.max_texture_dimension_2d);
    tracing::info!("Max Texture Dimension 3D: {}", limits.max_texture_dimension_3d);
    tracing::info!("Max Texture Array Layers: {}", limits.max_texture_array_layers);
    tracing::info!("Max Bind Groups: {}", limits.max_bind_groups);
    tracing::info!("Max Bindings Per Bind Group: {}", limits.max_bindings_per_bind_group);
    tracing::info!("Max Dynamic Uniform Buffers Per Pipeline Layout: {}", limits.max_dynamic_uniform_buffers_per_pipeline_layout);
    tracing::info!("Max Dynamic Storage Buffers Per Pipeline Layout: {}", limits.max_dynamic_storage_buffers_per_pipeline_layout);
    tracing::info!("Max Sampled Textures Per Shader Stage: {}", limits.max_sampled_textures_per_shader_stage);
    tracing::info!("Max Samplers Per Shader Stage: {}", limits.max_samplers_per_shader_stage);
    tracing::info!("Max Storage Buffers Per Shader Stage: {}", limits.max_storage_buffers_per_shader_stage);
    tracing::info!("Max Storage Textures Per Shader Stage: {}", limits.max_storage_textures_per_shader_stage);
    tracing::info!("Max Uniform Buffers Per Shader Stage: {}", limits.max_uniform_buffers_per_shader_stage);
    tracing::info!("Max Uniform Buffer Binding Size: {}", limits.max_uniform_buffer_binding_size);
    tracing::info!("Max Storage Buffer Binding Size: {}", limits.max_storage_buffer_binding_size);
    tracing::info!("Min Uniform Buffer Offset Alignment: {}", limits.min_uniform_buffer_offset_alignment);
    tracing::info!("Min Storage Buffer Offset Alignment: {}", limits.min_storage_buffer_offset_alignment);
    tracing::info!("Max Vertex Buffers: {}", limits.max_vertex_buffers);
    tracing::info!("Max Buffer Size: {}", limits.max_buffer_size);
    tracing::info!("Max Vertex Attributes: {}", limits.max_vertex_attributes);
    tracing::info!("Max Vertex Buffer Array Stride: {}", limits.max_vertex_buffer_array_stride);
    tracing::info!("Max InterStage Shader Components: {}", limits.max_inter_stage_shader_components);
    tracing::info!("Max Compute Workgroup Storage Size: {}", limits.max_compute_workgroup_storage_size);
    tracing::info!("Max Compute Invocations Per Workgroup: {}", limits.max_compute_invocations_per_workgroup);
    tracing::info!("Max Compute Workgroup Size X: {}", limits.max_compute_workgroup_size_x);
    tracing::info!("Max Compute Workgroup Size Y: {}", limits.max_compute_workgroup_size_y);
    tracing::info!("Max Compute Workgroup Size Z: {}", limits.max_compute_workgroup_size_z);
    tracing::info!("Max Compute Workgroups Per Dimension: {}", limits.max_compute_workgroups_per_dimension);
    tracing::info!("=====================================");
}
