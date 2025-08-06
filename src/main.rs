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
    println!("🧪 Testing generated message structs...");
    
    // Test ViewerEffect message creation
    test_viewer_effect_creation();
    
    println!("🎯 Generated message system validation complete!");
}

fn test_viewer_effect_creation() {
    use crate::networking::effects::{EffectManager, Position};
    use uuid::Uuid;
    
    println!("  📡 Testing ViewerEffect creation...");
    
    let mut effect_manager = EffectManager::new();
    let agent_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    
    // Test point-at effect (Type=9 as seen in hippolog)
    let source_pos = Position::new(100.0, 200.0, 50.0);
    let target_pos = Position::new(150.0, 250.0, 55.0);
    
    let viewer_effect = effect_manager.create_point_at_effect(
        agent_id,
        session_id,
        source_pos,
        target_pos
    );
    
    println!("    ✅ Created ViewerEffect message:");
    println!("       Agent ID: {}", agent_id);
    println!("       Session ID: {}", session_id);
    println!("       Effect count: {}", viewer_effect.effect.len());
    println!("       Effect type: {} (PointAt)", viewer_effect.effect[0].r#type);
    println!("       Duration: {} seconds", viewer_effect.effect[0].duration);
    println!("       Color bytes: {} bytes", viewer_effect.effect[0].color.len());
    println!("       TypeData bytes: {} bytes", viewer_effect.effect[0].type_data.data.len());
    
    // Test beam effect
    let beam_effect = effect_manager.create_beam_effect(
        agent_id,
        session_id,
        source_pos,
        target_pos
    );
    
    println!("    ✅ Created Beam effect:");
    println!("       Effect type: {} (Beam)", beam_effect.effect[0].r#type);
    println!("       Duration: {} seconds", beam_effect.effect[0].duration);
    
    println!("  ✅ ViewerEffect message creation test passed!");
}

fn main() -> eframe::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let _guard = runtime.enter();

    logging::init_logging();
    
    // Initialize packet registry for networking
    networking::packets::init_packet_registry();
    
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