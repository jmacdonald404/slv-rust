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
        ui::main_window::show_main_window(ctx, &mut self.ui_state);
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn test_generated_messages() {
    use crate::networking::protocol::{Message, messages::*};
    use uuid::Uuid;
    
    println!("🧪 Testing generated message structs...");
    
    // Test UseCircuitCode generation
    let agent_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let circuit_code = 12345u32;
    
    let use_circuit_msg = UseCircuitCode {
        code: circuit_code,
        session_id,
        id: agent_id.as_bytes().to_vec(),
    };
    
    println!("✅ UseCircuitCode: code={}, session_id={}, agent_id_len={}", 
        use_circuit_msg.code, use_circuit_msg.session_id, use_circuit_msg.id.len());
    
    // Test CompleteAgentMovement generation
    let complete_agent_msg = CompleteAgentMovement {
        agent_id,
        session_id,
        circuit_code,
    };
    
    println!("✅ CompleteAgentMovement: agent_id={}, session_id={}, circuit_code={}", 
        complete_agent_msg.agent_id, complete_agent_msg.session_id, complete_agent_msg.circuit_code);
    
    // Test RegionHandshakeReply generation
    let handshake_reply = RegionHandshakeReply {
        agent_id,
        session_id,
        flags: 0x01,
    };
    
    println!("✅ RegionHandshakeReply: agent_id={}, session_id={}, flags=0x{:02X}", 
        handshake_reply.agent_id, handshake_reply.session_id, handshake_reply.flags);
    
    // Test Message enum wrapping
    let _msg_enum = Message::UseCircuitCode(use_circuit_msg);
    println!("✅ Message enum wrapping successful");
    
    println!("🎯 Generated message system validation complete!");
}

fn main() -> eframe::Result<()> {
    logging::init_logging();
    // test stdout and stderr
    println!("PRINT TEST: If you see this, stdout works!");
    eprintln!("EPRINT TEST: If you see this, stderr works!");
    println!("VERSION: {}", VERSION);
    
    // SL_CODEC debug messages tested and working! 
    // Test our generated message system
    println!("=== TESTING GENERATED MESSAGE SYSTEM ===");
    networking::protocol::sl_compatibility::SLMessageCodec::test_debug_messages();
    
    // Test generated message creation
    println!("=== TESTING GENERATED HANDSHAKE MESSAGES ===");
    test_generated_messages();
    
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
