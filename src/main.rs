use slv_rust::rendering::engine::State as RenderState;
use slv_rust::ui::UiState;
use winit::event_loop::EventLoop;
use std::net::SocketAddr;
use slv_rust::networking::circuit::Circuit;
use std::sync::{Arc, Mutex};
use std::panic;
use tokio::runtime::Runtime;
use tracing_subscriber;
use tracing::info;

pub struct AppState<'a> {
    pub render_state: RenderState<'a>,
    pub ui_state: UiState,
}

impl<'a> AppState<'a> {
    pub async fn cleanup(&mut self) {
        if let Some(circuit) = self.ui_state.udp_circuit.as_mut() {
            if let Some(session) = &self.ui_state.login_state.session_info {
                if let Ok(ip) = session.sim_ip.parse() {
                    let sim_addr = SocketAddr::new(ip, session.sim_port);
                    circuit.disconnect_and_logout(&sim_addr).await;
                    self.ui_state.udp_circuit = None;
                    self.ui_state.udp_progress = slv_rust::ui::UdpConnectionProgress::NotStarted;
                    self.ui_state.login_state.status_message = "Disconnected from server.".to_string();
                }
            }
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
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
    let mut app_state_unwrapped = Arc::try_unwrap(app_state).expect("AppState Arc still has multiple owners").into_inner().unwrap();
    event_loop.run_app(&mut app_state_unwrapped).expect("Failed to run app");
}