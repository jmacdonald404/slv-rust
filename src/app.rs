use crate::rendering::engine::State as RenderState;
use crate::ui::UiState;
use std::net::SocketAddr;
use winit::event::{WindowEvent, Event};
use winit::event_loop::{EventLoop, ControlFlow};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::info;
use winit::application::ApplicationHandler;
use winit::event_loop::ActiveEventLoop;
use tokio::sync::Mutex;

pub struct AppState<'a> {
    pub render_state: RenderState<'a>,
    pub ui_state: UiState,
}

impl<'a> AppState<'a> {
    pub async fn cleanup(&mut self) {
        let mut should_clear_udp_circuit = false;
        if let Some(circuit_mutex) = self.ui_state.udp_circuit.as_mut() {
            if let Some(session) = &self.ui_state.login_state.session_info {
                if let Ok(ip) = session.sim_ip.parse() {
                    let sim_addr = SocketAddr::new(ip, session.sim_port);
                    {
                        let mut circuit = circuit_mutex.lock().await;
                        circuit.disconnect_and_logout(&sim_addr).await;
                    }
                    should_clear_udp_circuit = true;
                    self.ui_state.udp_progress = crate::ui::UdpConnectionProgress::NotStarted;
                    self.ui_state.login_state.status_message = "Disconnected from server.".to_string();
                }
            }
        }
        if should_clear_udp_circuit {
            self.ui_state.udp_circuit = None;
        }
    }
}

impl<'a> ApplicationHandler for AppState<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.render_state.renderer.is_none() {
            let window = Arc::new(event_loop.create_window(winit::window::Window::default_attributes().with_title("slv-rust")).expect("Failed to create window"));
            self.render_state.window = Some(window.clone());
            let renderer = pollster::block_on(crate::rendering::engine::RenderEngine::new(window));
            self.render_state.renderer = Some(renderer);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: winit::window::WindowId, event: WindowEvent) {
        if self.render_state.window.as_ref().map_or(true, |w| w.id() != window_id) {
            return;
        }
        if let Some(renderer) = self.render_state.renderer.as_mut() {
            if !renderer.camera_controller.process_events(&event) {
                match event {
                    WindowEvent::CloseRequested => {
                        // Call coordinated cleanup before exit
                        pollster::block_on(self.cleanup());
                        event_loop.exit();
                    },
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(physical_size);
                    },
                    WindowEvent::RedrawRequested => {
                        renderer.camera_controller.update_camera(&mut renderer.camera);
                        let camera_uniform = crate::rendering::camera_uniform::CameraUniform {
                            view_proj: renderer.camera.build_view_projection_matrix().into(),
                        };
                        renderer.queue.write_buffer(&renderer.uniform_buffer, 0, bytemuck::cast_slice(&[camera_uniform]));
                        if renderer.light.position != self.render_state.last_light_position {
                            let light_uniform = renderer.light.to_uniform();
                            renderer.queue.write_buffer(&renderer.light_uniform_buffer, 0, bytemuck::cast_slice(&[light_uniform]));
                            self.render_state.last_light_position = renderer.light.position;
                        }
                        renderer.render_frame();
                        if let Some(window) = self.render_state.window.as_ref() {
                            window.request_redraw();
                        }
                    }
                    _ => {}
                }
            }
        }
        // After handling the event, check for logout request
        if self.ui_state.logout_requested {
            pollster::block_on(self.cleanup());
            self.ui_state.logout_requested = false;
        }
    }
}
