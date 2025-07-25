use crate::ui::UiState;
use std::sync::Arc;
use std::net::SocketAddr;

/*
// New: Main-thread-only RenderContext for wgpu/winit fields
pub struct RenderContext<'a> {
    pub surface: wgpu::Surface<'a>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub window: Arc<winit::window::Window>,
    pub ui_ctx: Option<crate::ui::UiContext>,
}
*/

// Multithreaded shared state for UI/logic
pub struct AppState {
    pub ui_state: UiState,
    // Add other Send + Sync fields as needed
}

// impl AppState {
//     pub async fn cleanup(&mut self) {
//         let mut should_clear_udp_circuit = false;
//         if let Some(circuit_mutex) = self.ui_state.udp_circuit.as_mut() {
//             if let Some(session) = &self.ui_state.login_state.session_info {
//                 if let Ok(ip) = session.sim_ip.parse() {
//                     let sim_addr = SocketAddr::new(ip, session.sim_port);
//                     {
//                         let mut circuit = circuit_mutex.lock().await;
//                         circuit.disconnect_and_logout(&sim_addr).await;
//                     }
//                     should_clear_udp_circuit = true;
//                     self.ui_state.udp_progress = crate::ui::UdpConnectionProgress::NotStarted;
//                     self.ui_state.login_state.status_message = "Disconnected from server.".to_string();
//                 }
//             }
//         }
//         if should_clear_udp_circuit {
//             self.ui_state.udp_circuit = None;
//         }
//     }
// }
