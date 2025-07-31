use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};
use std::env;
use std::fs;
use std::io;

/// Initialize logging with comprehensive configuration
pub fn init_logging() {
    // Check for environment variables to configure logging
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let enable_wgpu_logging = env::var("WGPU_LOG").unwrap_or_else(|_| "1".to_string()) == "1";
    let enable_backtrace = env::var("RUST_BACKTRACE").unwrap_or_else(|_| "0".to_string()) == "1";

    // Remove existing log.txt file if it exists
    if let Err(e) = fs::remove_file("log.txt") {
        if e.kind() != io::ErrorKind::NotFound {
            eprintln!("Warning: Failed to remove existing log.txt: {}", e);
        }
    }

    // Create log file
    let log_file = fs::File::create("log.txt").expect("Failed to create log.txt");

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
            // filter = filter.add_directive("winit=debug".parse().unwrap());
            filter = filter.add_directive("pollster=debug".parse().unwrap());
            
            filter
        });

    // Create the subscriber with multiple layers (console + file)
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
        )
        .with(fmt::layer()
            .with_writer(log_file)
            .with_span_events(FmtSpan::CLOSE)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false) // No ANSI codes in file
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
    tracing::info!("File logging enabled: log.txt (session-based, cleaned on startup)");
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
    let _limits = device.limits();
    let _features = device.features();
    
    tracing::info!("=== WGPU Device Information ===");
    // Device info logging temporarily disabled to reduce verbosity
    tracing::info!("=====================================");
}
