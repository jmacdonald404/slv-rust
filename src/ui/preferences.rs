// TODO: Integrate preferences/settings UI with egui
// TODO: Implement settings panels and controls

use egui::Context;
use crate::ui::PreferencesState;

pub fn show_preferences_panel(ctx: &Context, prefs: &mut PreferencesState) {
    egui::Window::new("Preferences").show(ctx, |ui| {
        ui.label("Settings:");
        ui.checkbox(&mut prefs.enable_sound, "Enable Sound");
        ui.add(egui::Slider::new(&mut prefs.volume, 0.0..=1.0).text("Volume"));
        // TODO: Add more settings controls (graphics, UI, etc.)
    });
}
