use tokio; // Add this import for the runtime
// use crate::utils::lludp::{LluPacket, LluPacketFlags, build_use_circuit_code_packet};
use crate::utils::logging;

mod utils;
mod networking;
mod ui;
// mod assets;
// mod rendering;
mod config;
// mod world;
mod app;

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
    logging::init_logging();
    // test stdout and stderr
    println!("PRINT TEST: If you see this, stdout works!");
    eprintln!("EPRINT TEST: If you see this, stderr works!");
    println!("VERSION: {}", VERSION);
    
    // Build a multi-threaded Tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    // Run the eframe app inside the runtime
    runtime.block_on(async {
        eframe::run_native(
            &format!("holy f*ckles it's sonic and knuckles {}", VERSION),
            eframe::NativeOptions::default(),
            Box::new(|_cc| Ok(Box::new(MyApp::default()))),
        )
    })
}
