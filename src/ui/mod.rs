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
use crate::config::settings;
use crate::ui::proxy::ProxySettings;

#[derive(Debug, Clone, Default)]
pub struct AgentState {
    pub can_modify_navmesh: bool,
    pub has_modified_navmesh: bool,
    pub god_level: i32,
    pub hover_height: f32,
    pub language: String,
    pub language_is_public: bool,
    pub access_prefs_max: String,
    pub default_object_perm_masks: (i32, i32, i32), // (Everyone, Group, NextOwner)
}

// New organized structure
pub mod app;
pub mod login;
pub mod main_app;
pub mod components;

// Legacy modules (kept for compatibility)
pub mod main_window;
pub mod chat;
pub mod inventory;
pub mod preferences;
pub mod proxy;

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
    pub selected_grid: crate::networking::auth::Grid,
    pub status_message: String,
    pub prefs_modal_open: bool,
    pub session_info: Option<()>,
    pub agree_to_tos_next_login: bool,
    pub read_critical_next_login: bool, // Track if user must accept critical message
}

impl Default for LoginState {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            selected_grid: crate::networking::auth::Grid::default(),
            status_message: String::new(),
            prefs_modal_open: false,
            session_info: None,
            agree_to_tos_next_login: false,
            read_critical_next_login: false,
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
    pub result: Result<(), String>,
}

pub enum UdpConnectionProgress {
    NotStarted,
    Connecting,
    Connected,
    Error(String),
}

pub enum UiEvent {
    ShowTos {
        tos_id: String,
        tos_html: String,
        message: String,
    },
    AgentStateUpdate(String),
    InWorldReady, // <-- Add this
    // Add more events as needed
}

pub struct UiState {
    pub runtime_handle: tokio::runtime::Handle,
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
    pub udp_circuit: Option<()>,
    pub udp_progress: UdpConnectionProgress,
    pub udp_connect_tx: Sender<()>,
    pub udp_connect_rx: Receiver<()>,
    pub udp_connect_task: Option<JoinHandle<()>>,
    pub logout_requested: bool,
    pub chat_event_tx: Option<Sender<(String, String)>>,
    pub chat_event_rx: Option<Receiver<(String, String)>>,
    pub proxy_settings: ProxySettings,
    pub tos_required: bool,
    pub tos_html: Option<String>,
    pub tos_id: Option<String>,
    pub tos_message: Option<String>,
    pub ui_event_rx: crossbeam_channel::Receiver<UiEvent>,
    pub ui_event_tx: crossbeam_channel::Sender<UiEvent>,
    pub agent_state: Option<AgentState>,
    pub session_udp_port: u16,
}

pub struct PreferencesState {
    pub enable_sound: bool,
    pub volume: f32,
    pub graphics_api: String,
    pub vsync: bool,
    pub render_distance: u32,
    pub max_bandwidth: u32,
    pub timeout: u32,
    // UDP test fields
    pub udp_test_result: Option<String>,
    pub udp_test_in_progress: bool,
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
            udp_test_result: None,
            udp_test_in_progress: false,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        let (login_result_tx, login_result_rx) = unbounded();
        let (udp_connect_tx, udp_connect_rx) = unbounded();
        let (ui_event_tx, ui_event_rx) = unbounded();
        let mut preferences = PreferencesState::default();
        let mut proxy_settings = ProxySettings::default();
        if let Some((prefs, proxy)) = settings::load_general_settings() {
            preferences = prefs;
            proxy_settings = proxy;
        } else if let Some(loaded) = settings::load_preferences() {
            preferences = loaded;
        }
        // Generate a random free 5-digit UDP port for the session
        let session_udp_port = 0;
        Self {
            runtime_handle: tokio::runtime::Handle::current(),
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
            proxy_settings,
            tos_required: false,
            tos_html: None,
            tos_id: None,
            tos_message: None,
            ui_event_rx,
            ui_event_tx,
            agent_state: None,
            session_udp_port,
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
    for (id, _image_delta) in &eframe::egui::TexturesDelta::default().set {
        // The original UiRenderer had a renderer here, but it's removed.
        // This part of the logic needs to be re-evaluated or removed if not directly applicable.
        // For now, we'll keep it as is, but it might need adjustment depending on how textures are managed.
        // Since UiRenderer is removed, this loop will effectively do nothing.
        // If textures are managed by eframe's internal state, this loop might be redundant or need a different approach.
        // For now, we'll just log a message.
        // tracing::warn!("Texture update for ID: {:?} (image_delta: <not debug>) - UiRenderer removed, texture updates will not be applied.", id);
    }
    for id in &eframe::egui::TexturesDelta::default().free {
        // Similarly, UiRenderer was removed.
        // tracing::warn!("Texture free for ID: {:?} - UiRenderer removed, texture free will not be applied.", id);
    }

    // Draw egui frame
    // The original UiRenderer had a paint method here.
    // Since UiRenderer is removed, this call will effectively do nothing.
    // If eframe's egui context itself handles rendering, this might be redundant or need a different approach.
    // For now, we'll just log a message.
    // tracing::warn!("UiRenderer::paint called - UiRenderer removed, rendering will not be performed.");
}

// TODO: Add stubs for future UI state (e.g., avatar, object selection, notifications)
