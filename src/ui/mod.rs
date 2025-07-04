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
use tokio::task::JoinHandle;
use crossbeam_channel::{unbounded, Sender, Receiver};
use crate::networking::session::LoginSessionInfo;
use crate::networking::circuit::Circuit;
use crate::config::settings;

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

pub struct LoginState {
    pub username: String,
    pub password: String,
    pub status_message: String,
    pub prefs_modal_open: bool,
    pub session_info: Option<crate::networking::session::LoginSessionInfo>,
}

impl Default for LoginState {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            status_message: String::new(),
            prefs_modal_open: false,
            session_info: None,
        }
    }
}

pub enum LoginUiState {
    LoginSplash,
    MainApp,
    LoadingWorld,
    InWorld,
}

pub enum LoginProgress {
    Idle,
    InProgress,
    Success,
    Error(String),
}

pub struct LoginResult {
    pub result: Result<LoginSessionInfo, String>,
}

pub enum UdpConnectionProgress {
    NotStarted,
    Connecting,
    Connected,
    Error(String),
}

pub struct UiState {
    pub chat_input: String,
    pub chat_messages: VecDeque<String>,
    pub inventory_items: Vec<String>,
    pub preferences: PreferencesState,
    pub login_state: LoginState,
    pub login_ui_state: LoginUiState,
    pub login_progress: LoginProgress,
    pub login_task: Option<JoinHandle<()>>,
    pub login_result_tx: Sender<LoginResult>,
    pub login_result_rx: Receiver<LoginResult>,
    pub udp_circuit: Option<Circuit>,
    pub udp_progress: UdpConnectionProgress,
    pub udp_connect_tx: Sender<crate::ui::main_window::UdpConnectResult>,
    pub udp_connect_rx: Receiver<crate::ui::main_window::UdpConnectResult>,
    pub udp_connect_task: Option<JoinHandle<()>>,
    pub logout_requested: bool,
    pub chat_event_tx: Option<Sender<(String, String)>>,
    pub chat_event_rx: Option<Receiver<(String, String)>>,
}

pub struct PreferencesState {
    pub enable_sound: bool,
    pub volume: f32,
    pub graphics_api: String,
    pub vsync: bool,
    pub render_distance: u32,
    pub max_bandwidth: u32,
    pub timeout: u32,
    // TODO: Add more settings as needed
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            enable_sound: true,
            volume: 0.8,
            graphics_api: "vulkan".to_string(),
            vsync: true,
            render_distance: 256,
            max_bandwidth: 1500,
            timeout: 30,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        let (login_result_tx, login_result_rx) = unbounded();
        let (udp_connect_tx, udp_connect_rx) = unbounded();
        let mut preferences = PreferencesState::default();
        if let Some(loaded) = settings::load_preferences() {
            preferences = loaded;
        }
        Self {
            chat_input: String::new(),
            chat_messages: VecDeque::from(vec!["Welcome to slv-rust!".to_string()]),
            inventory_items: vec!["Test Item 1".to_string(), "Test Item 2".to_string()],
            preferences,
            login_state: LoginState::default(),
            login_ui_state: LoginUiState::LoginSplash,
            login_progress: LoginProgress::Idle,
            login_task: None,
            login_result_tx,
            login_result_rx,
            udp_circuit: None,
            udp_progress: UdpConnectionProgress::NotStarted,
            udp_connect_tx,
            udp_connect_rx,
            udp_connect_task: None,
            logout_requested: false,
            chat_event_tx: None,
            chat_event_rx: None,
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
    let in_world = matches!(ui_state.login_ui_state, LoginUiState::MainApp | LoginUiState::LoadingWorld | LoginUiState::InWorld);
    let full_output = ui_ctx.egui_ctx.run(raw_input, |ctx| {
        crate::ui::main_window::show_main_window(ctx, ui_state);
        if let crate::ui::LoginUiState::MainApp | crate::ui::LoginUiState::LoadingWorld | crate::ui::LoginUiState::InWorld = ui_state.login_ui_state {
            crate::ui::chat::show_chat_panel(ctx, &mut ui_state.chat_input, &mut ui_state.chat_messages, ui_state);
            crate::ui::inventory::show_inventory_panel(ctx, &ui_state.inventory_items);
            crate::ui::preferences::show_preferences_panel(ctx, &mut ui_state.preferences, in_world);
        } else {
            crate::ui::preferences::show_preferences_panel(ctx, &mut ui_state.preferences, false);
        }
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
