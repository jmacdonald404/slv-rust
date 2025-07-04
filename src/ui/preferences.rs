// TODO: Integrate preferences/settings UI with egui
// TODO: Implement settings panels and controls

use egui::Context;
use crate::ui::PreferencesState;
use crate::config::settings;

pub fn show_preferences_panel(ctx: &Context, prefs: &mut PreferencesState, in_world: bool) {
    let mut changed = false;
    egui::Window::new("Preferences").show(ctx, |ui| {
        ui.label("Settings:");
        changed |= ui.checkbox(&mut prefs.enable_sound, "Enable Sound").changed();
        changed |= ui.add(egui::Slider::new(&mut prefs.volume, 0.0..=1.0).text("Volume")).changed();
        if in_world {
            ui.separator();
            ui.label("Graphics:");
            changed |= egui::ComboBox::from_label("Graphics API")
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
            changed |= ui.add(egui::Slider::new(&mut prefs.render_distance, 64..=512).text("Render Distance (m)")).changed();
            ui.separator();
            ui.label("Network:");
            changed |= ui.add(egui::Slider::new(&mut prefs.max_bandwidth, 500..=5000).text("Max Bandwidth (KB/s)")).changed();
            changed |= ui.add(egui::Slider::new(&mut prefs.timeout, 5..=120).text("Timeout (s)")).changed();
        }
    });
    if changed {
        let _ = settings::save_preferences(prefs);
    }
}
