// TODO: Integrate egui for immediate-mode GUI
// TODO: Set up egui context and UI state management
// TODO: Implement main UI event loop and rendering
// TODO: Add modules for HUD, settings, chat, inventory, preferences

use egui_winit::State as EguiWinitState;
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use egui::{Context as EguiContext, ViewportId};
use wgpu::{Device, Queue, SurfaceConfiguration, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;
use std::collections::VecDeque;

pub mod main_window;
pub mod chat;
pub mod inventory;
pub mod preferences;

pub struct UiContext {
    pub egui_ctx: EguiContext,
    pub egui_winit: EguiWinitState,
}

impl UiContext {
    pub fn new(window: &Window) -> Self {
        let egui_ctx = EguiContext::default();
        let viewport_id = ViewportId::ROOT;
        let egui_winit = EguiWinitState::new(egui_ctx.clone(), viewport_id, &window, None, None, None);
        Self { egui_ctx, egui_winit }
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.egui_winit.on_window_event(&window, event).consumed
    }
}

pub struct UiRenderer {
    pub renderer: EguiRenderer,
}

impl UiRenderer {
    pub fn new(device: &Device, _surface_config: &SurfaceConfiguration, format: wgpu::TextureFormat) -> Self {
        let renderer = EguiRenderer::new(device, format, None, 1, false);
        Self { renderer }
    }

    pub fn paint(
        &mut self,
        device: &Device,
        queue: &Queue,
        surface_config: &SurfaceConfiguration,
        view: &TextureView,
        window: &Window,
        egui_ctx: &EguiContext,
        full_output: egui::FullOutput,
    ) {
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: window.scale_factor() as f32,
        };
        let paint_jobs = egui_ctx.tessellate(full_output.shapes, screen_descriptor.pixels_per_point);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui UI Encoder"),
        });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = render_pass.forget_lifetime();
            self.renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }
        queue.submit(Some(encoder.finish()));
    }
}

pub struct UiState {
    pub chat_input: String,
    pub chat_messages: VecDeque<String>,
    pub inventory_items: Vec<String>,
    pub preferences: PreferencesState,
}

pub struct PreferencesState {
    pub enable_sound: bool,
    pub volume: f32,
    // TODO: Add more settings as needed
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            chat_input: String::new(),
            chat_messages: VecDeque::from(vec!["Welcome to slv-rust!".to_string()]),
            inventory_items: vec!["Test Item 1".to_string(), "Test Item 2".to_string()],
            preferences: PreferencesState {
                enable_sound: true,
                volume: 0.8,
            },
        }
    }
}

/// Run the main UI frame. Call this from your render loop.
pub fn run_ui_frame(
    ui_ctx: &mut UiContext,
    ui_renderer: &mut UiRenderer,
    device: &Device,
    queue: &Queue,
    surface_config: &SurfaceConfiguration,
    view: &TextureView,
    window: &Window,
    ui_state: &mut UiState,
) {
    // Begin egui frame
    let raw_input = ui_ctx.egui_winit.take_egui_input(window);
    let full_output = ui_ctx.egui_ctx.run(raw_input, |ctx| {
        crate::ui::main_window::show_main_window(ctx);
        crate::ui::chat::show_chat_panel(ctx, &mut ui_state.chat_input, &mut ui_state.chat_messages);
        crate::ui::inventory::show_inventory_panel(ctx, &ui_state.inventory_items);
        crate::ui::preferences::show_preferences_panel(ctx, &mut ui_state.preferences);
    });

    // Draw egui frame
    ui_renderer.paint(
        device,
        queue,
        surface_config,
        view,
        window,
        &ui_ctx.egui_ctx,
        full_output,
    );
}

// TODO: Add stubs for future UI state (e.g., avatar, object selection, notifications)
