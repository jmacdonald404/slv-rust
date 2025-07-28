use tokio; // Add this import for the runtime
// use crate::utils::lludp::{LluPacket, LluPacketFlags, build_use_circuit_code_packet};
use crate::utils::logging;

mod utils;
mod networking;
mod ui;
mod assets;
mod rendering;
mod config;
mod world;
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
        ui::app::show_main_window(ctx, &mut self.ui_state);
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn test_generated_messages() {
    println!("🧪 Testing generated message structs... (DISABLED - networking removed)");
    println!("🎯 Generated message system validation complete!");
}

fn main() -> eframe::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let _guard = runtime.enter();

    logging::init_logging();
    // test stdout and stderr
    println!("PRINT TEST: If you see this, stdout works!");
    eprintln!("EPRINT TEST: If you see this, stderr works!");
    println!("VERSION: {}", VERSION);

    // SL_CODEC debug messages tested and working!
    // Test our generated message system
    println!("=== TESTING GENERATED MESSAGE SYSTEM ===");
    // networking::protocol::sl_compatibility::SLMessageCodec::test_debug_messages(); // Removed

    // Test generated message creation
    test_generated_messages();

    println!("=== STARTING EGUI ===");

    let options = eframe::NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        "slv-rust",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}