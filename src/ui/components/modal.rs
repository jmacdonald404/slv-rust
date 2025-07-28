use eframe::egui;
use egui::{Align2, Vec2};

pub struct ModalConfig {
    pub title: String,
    pub min_width: f32,
    pub collapsible: bool,
    pub resizable: bool,
    pub anchor: Align2,
}

impl Default for ModalConfig {
    fn default() -> Self {
        Self {
            title: "Modal".to_string(),
            min_width: 400.0,
            collapsible: false,
            resizable: false,
            anchor: Align2::CENTER_CENTER,
        }
    }
}

pub fn show_modal<R>(
    ctx: &egui::Context,
    config: ModalConfig,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let mut result = None;
    
    egui::Window::new(config.title)
        .anchor(config.anchor, Vec2::ZERO)
        .collapsible(config.collapsible)
        .resizable(config.resizable)
        .show(ctx, |ui| {
            ui.set_min_width(config.min_width);
            result = Some(content(ui));
        });
    
    result
}

pub fn show_confirmation_dialog(
    ctx: &egui::Context,
    title: &str,
    message: &str,
    open: &mut bool,
) -> Option<bool> {
    if !*open {
        return None;
    }
    
    let mut result = None;
    
    egui::Window::new(title)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            
            ui.label(message);
            ui.add_space(15.0);
            
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    result = Some(true);
                    *open = false;
                }
                if ui.button("No").clicked() {
                    result = Some(false);
                    *open = false;
                }
            });
        });
    
    result
}