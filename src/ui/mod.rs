// TODO: Integrate egui for immediate-mode GUI
// TODO: Set up egui context and UI state management
// TODO: Implement main UI event loop and rendering
// TODO: Add modules for HUD, settings, chat, inventory, preferences

use eframe::egui::{Context as EguiContext, ViewportId};
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
}

impl UiContext {
    pub fn new(_window: &Window) -> Self {
        let egui_ctx = EguiContext::default();
        Self { egui_ctx }
    }
    pub fn handle_event(&mut self, _window: &Window, _event: &WindowEvent) -> bool {
        false // No egui_winit state to manage events
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

#[derive(PartialEq)]
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
    pub udp_circuit: Option<std::sync::Arc<tokio::sync::Mutex<Circuit>>>,
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
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface_config: &wgpu::SurfaceConfiguration,
    view: &wgpu::TextureView,
    window: &winit::window::Window,
    _ui_state: &mut UiState,
) {
    // Begin egui frame
    // Remove this line and any related manual frame/input handling:
    // let full_output = ui_ctx.egui_ctx.run(eframe::egui::RawInput::from_window_event(window, &ui_ctx.egui_ctx), |ctx| {
    //     eframe::egui::CentralPanel::default().show(ctx, |ui| {
    //         ui.label("Hello, egui!");
    //     });
    // });

    // Apply texture updates (font atlas, user textures)
    for (id, image_delta) in &eframe::egui::TexturesDelta::default().set {
        // The original UiRenderer had a renderer here, but it's removed.
        // This part of the logic needs to be re-evaluated or removed if not directly applicable.
        // For now, we'll keep it as is, but it might need adjustment depending on how textures are managed.
        // Since UiRenderer is removed, this loop will effectively do nothing.
        // If textures are managed by eframe's internal state, this loop might be redundant or need a different approach.
        // For now, we'll just log a message.
        tracing::warn!("Texture update for ID: {:?} (image_delta: <not debug>) - UiRenderer removed, texture updates will not be applied.", id);
    }
    for id in &eframe::egui::TexturesDelta::default().free {
        // Similarly, UiRenderer was removed.
        tracing::warn!("Texture free for ID: {:?} - UiRenderer removed, texture free will not be applied.", id);
    }

    // Draw egui frame
    // The original UiRenderer had a paint method here.
    // Since UiRenderer is removed, this call will effectively do nothing.
    // If eframe's egui context itself handles rendering, this might be redundant or need a different approach.
    // For now, we'll just log a message.
    tracing::warn!("UiRenderer::paint called - UiRenderer removed, rendering will not be performed.");
}

// TODO: Add stubs for future UI state (e.g., avatar, object selection, notifications)
