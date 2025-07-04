// TODO: Integrate egui for immediate-mode GUI
// TODO: Set up egui context and UI state management
// TODO: Implement main UI event loop and rendering
// TODO: Add modules for HUD, settings, chat, inventory, preferences

use egui_winit::State as EguiWinitState;
use egui_wgpu::renderer::ScreenDescriptor;
use egui::{Context as EguiContext};
use winit::event::WindowEvent;
use wgpu::{Device, Queue, SurfaceConfiguration, TextureView};
use egui_wgpu::Renderer as EguiRenderer;
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
    pub fn new(window: &winit::window::Window) -> Self {
        let egui_ctx = EguiContext::default();
        let egui_winit = EguiWinitState::new(window);
        Self { egui_ctx, egui_winit }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        self.egui_winit.on_event(&self.egui_ctx, event).consumed
    }
}

pub struct UiRenderer {
    pub renderer: EguiRenderer,
}

impl UiRenderer {
    pub fn new(device: &Device, surface_config: &SurfaceConfiguration, format: wgpu::TextureFormat) -> Self {
        let renderer = EguiRenderer::new(device, format, None, 1);
        Self { renderer }
    }

    pub fn paint(
        &mut self,
        device: &Device,
        queue: &Queue,
        surface_config: &SurfaceConfiguration,
        view: &TextureView,
        egui_ctx: &EguiContext,
        full_output: &egui::FullOutput,
    ) {
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: surface_config.width as f32 / surface_config.width as f32, // TODO: Use actual scale factor
        };
        let paint_jobs = egui_ctx.tessellate(full_output.shapes.clone());
        self.renderer.update_buffers(
            device,
            queue,
            &paint_jobs,
            &full_output.textures_delta,
            &screen_descriptor,
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui UI Encoder"),
        });
        self.renderer.render(
            &mut encoder,
            view,
            &paint_jobs,
            &screen_descriptor,
            None,
        );
        queue.submit(Some(encoder.finish()));
        self.renderer.free_unused_textures();
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
    ui_state: &mut UiState,
) {
    // Begin egui frame
    let raw_input = ui_ctx.egui_winit.take_egui_input(&ui_ctx.egui_ctx);
    let full_output = ui_ctx.egui_ctx.run(raw_input, |ctx| {
        crate::ui::main_window::show_main_window(ctx);
        crate::ui::chat::show_chat_panel(ctx, &mut ui_state.chat_input, &mut ui_state.chat_messages);
        crate::ui::inventory::show_inventory_panel(ctx, &ui_state.inventory_items);
        crate::ui::preferences::show_preferences_panel(ctx, &mut ui_state.preferences);
    });
    ui_renderer.paint(device, queue, surface_config, view, &ui_ctx.egui_ctx, &full_output);
}

// TODO: Add stubs for future UI state (e.g., avatar, object selection, notifications)
