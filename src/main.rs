use winit::event_loop::EventLoop;
use winit::event::{Event, WindowEvent};
use tracing::info;
use slv_rust::utils::logging::init_logging;
use wgpu::{Instance, Backends, SurfaceConfiguration, TextureUsages, PresentMode, CompositeAlphaMode};
use winit::window::Window;
use std::sync::Arc;

mod networking;
mod config;
mod ui;

struct MyApp {
    ui_state: ui::UiState,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            ui_state: ui::UiState::default(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ui::main_window::show_main_window(ctx, &mut self.ui_state);
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> eframe::Result<()> {
    eframe::run_native(
        &format!("shrekondlyfe rust viewer {}", VERSION),
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}
