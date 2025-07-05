use slv_rust::rendering::engine::State as RenderState;
use slv_rust::ui::UiState;
use winit::event_loop::EventLoop;
use std::sync::{Arc, Mutex};
use std::panic;
use tokio::runtime::Runtime;
use tracing::info;
use slv_rust::app::AppState;
use slv_rust::utils::logging::{init_logging, log_system_info};
use tokio::signal;

fn main() {
    // Initialize comprehensive logging
    init_logging();
    
    // Log system information
    log_system_info();
    
    info!("slv-rust starting up");
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let render_state = RenderState {
        renderer: None,
        last_light_position: cgmath::Point3::new(0.0, 0.0, 0.0),
        window: None,
    };
    let ui_state = UiState::default();
    let app_state = Arc::new(Mutex::new(AppState {
        render_state,
        ui_state,
    }));
    // Set panic hook for cleanup
    let app_state_clone = Arc::clone(&app_state);
    panic::set_hook(Box::new(move |info| {
        if let Ok(mut state) = app_state_clone.lock() {
            let rt = Runtime::new().unwrap();
            rt.block_on(state.cleanup());
        }
        eprintln!("Panic occurred: {}", info);
    }));
    // Spawn shutdown handler for ctrl+c
    let app_state_shutdown = Arc::clone(&app_state);
    std::thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
            if let Ok(mut state) = app_state_shutdown.lock() {
                state.cleanup().await;
            }
            println!("Graceful shutdown: UDP circuit disconnected.");
            std::process::exit(0);
        });
    });
    // Run the app
    let mut app_state_guard = app_state.lock().unwrap();
    event_loop.run_app(&mut *app_state_guard).expect("Failed to run app");
}
