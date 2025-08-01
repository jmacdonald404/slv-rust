// TODO: Integrate preferences/settings UI with egui
// TODO: Implement settings panels and controls

use eframe::egui::Context;
use crate::ui::PreferencesState;
use crate::config::settings;
use std::sync::mpsc::{channel, TryRecvError};

pub fn show_preferences_panel(ctx: &eframe::egui::Context, prefs: &mut PreferencesState, _in_world: bool) {
    // --- UDP Test Result Channel ---
    static mut UDP_TEST_RESULT_RX: Option<std::sync::mpsc::Receiver<String>> = None;

    // Poll for result from background thread
    unsafe {
        if let Some(rx) = &UDP_TEST_RESULT_RX {
            match rx.try_recv() {
                Ok(result) => {
                    prefs.udp_test_in_progress = false;
                    prefs.udp_test_result = Some(result);
                    UDP_TEST_RESULT_RX = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    prefs.udp_test_in_progress = false;
                    UDP_TEST_RESULT_RX = None;
                }
            }
        }
    }
    let mut changed = false;
    eframe::egui::Window::new("Preferences").show(ctx, |ui| {
        ui.label("Settings:");
        changed |= ui.checkbox(&mut prefs.enable_sound, "Enable Sound").changed();
        changed |= ui.add(eframe::egui::Slider::new(&mut prefs.volume, 0.0..=1.0).text("Volume")).changed();
        ui.separator();
        ui.label("Graphics:");
        changed |= eframe::egui::ComboBox::from_label("Graphics API")
            .selected_text(&prefs.graphics_api)
            .show_ui(ui, |ui| {
                let mut c = false;
                c |= ui.selectable_value(&mut prefs.graphics_api, "vulkan".to_string(), "Vulkan").changed();
                c |= ui.selectable_value(&mut prefs.graphics_api, "dx12".to_string(), "DirectX 12").changed();
                c |= ui.selectable_value(&mut prefs.graphics_api, "metal".to_string(), "Metal").changed();
                c |= ui.selectable_value(&mut prefs.graphics_api, "opengl".to_string(), "OpenGL").changed();
                c
            }).inner.unwrap_or(false);
        changed |= ui.checkbox(&mut prefs.vsync, "VSync").changed();
        changed |= ui.add(eframe::egui::Slider::new(&mut prefs.render_distance, 64..=512).text("Render Distance (m)")).changed();
        ui.separator();
        ui.label("Network:");
        changed |= ui.add(eframe::egui::Slider::new(&mut prefs.max_bandwidth, 500..=5000).text("Max Bandwidth (KB/s)")).changed();
        changed |= ui.add(eframe::egui::Slider::new(&mut prefs.timeout, 5..=120).text("Timeout (s)")).changed();
        
        ui.separator();
        ui.label("Proxy Settings (Hippolyzer Support):");
        // Note: This version uses a simpler proxy configuration
        // Full proxy settings are available in the main app preferences
        // --- UDP Test Button ---
        if ui.button("Test UDP Send").clicked() && !prefs.udp_test_in_progress {
            prefs.udp_test_in_progress = true;
            prefs.udp_test_result = None;
            let (tx, rx) = std::sync::mpsc::channel();
            unsafe { UDP_TEST_RESULT_RX = Some(rx); }
            std::thread::spawn(move || {
                use std::net::UdpSocket;
                use std::time::Duration;
                let addr = "127.0.0.1:54321";
                let msg = b"slv-rust test";
                let result = match UdpSocket::bind("0.0.0.0:0") {
                    Ok(socket) => {
                        socket.set_write_timeout(Some(Duration::from_secs(1))).ok();
                        match socket.send_to(msg, addr) {
                            Ok(sent) => format!("UDP sent {} bytes to {}", sent, addr),
                            Err(e) => format!("UDP send error: {}", e),
                        }
                    }
                    Err(e) => format!("UDP bind error: {}", e),
                };
                let _ = tx.send(result);
            });
        }
        // --- Async UDP Test Button ---
        if ui.button("Test Async UDP Send").clicked() && !prefs.udp_test_in_progress {
            prefs.udp_test_in_progress = true;
            prefs.udp_test_result = None;
            let (tx, rx) = std::sync::mpsc::channel();
            unsafe { UDP_TEST_RESULT_RX = Some(rx); }
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = tx.send(format!("Tokio runtime error: {}", e));
                        return;
                    }
                };
                let result = rt.block_on(async {
                    use tokio::net::UdpSocket;
                    use std::time::Duration;
                    use tokio::time::timeout;
                    let addr = "127.0.0.1:54322";
                    let msg = b"slv-rust async test";
                    match UdpSocket::bind("0.0.0.0:0").await {
                        Ok(socket) => {
                            // Set a timeout for send
                            match timeout(Duration::from_secs(1), socket.send_to(msg, addr)).await {
                                Ok(Ok(sent)) => format!("Async UDP sent {} bytes to {}", sent, addr),
                                Ok(Err(e)) => format!("Async UDP send error: {}", e),
                                Err(_) => format!("Async UDP send timed out"),
                            }
                        }
                        Err(e) => format!("Async UDP bind error: {}", e),
                    }
                });
                let _ = tx.send(result);
            });
        }
        if let Some(ref result) = prefs.udp_test_result {
            ui.label(format!("UDP Test Result: {}", result));
        } else if prefs.udp_test_in_progress {
            ui.label("UDP test in progress...");
        }
    });
    if changed {
        let _ = settings::save_preferences(prefs);
    }
}
