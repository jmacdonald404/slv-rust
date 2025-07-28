use eframe::egui;
use egui::{Color32, Vec2};

pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
    Success,
}

impl ButtonStyle {
    fn color(&self) -> Color32 {
        match self {
            ButtonStyle::Primary => Color32::from_rgb(70, 130, 180),
            ButtonStyle::Secondary => Color32::GRAY,
            ButtonStyle::Danger => Color32::from_rgb(220, 53, 69),
            ButtonStyle::Success => Color32::from_rgb(40, 167, 69),
        }
    }
}

pub struct StyledButton {
    text: String,
    style: ButtonStyle,
    min_size: Option<Vec2>,
    enabled: bool,
}

impl StyledButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: ButtonStyle::Primary,
            min_size: None,
            enabled: true,
        }
    }
    
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
    
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = Some(size);
        self
    }
    
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let mut button = egui::Button::new(&self.text);
        
        if let Some(size) = self.min_size {
            button = button.min_size(size);
        }
        
        // For now, use default styling - custom colors can be added later
        ui.add_enabled(self.enabled, button)
    }
}

pub fn loading_button(ui: &mut egui::Ui, text: &str, loading: bool) -> egui::Response {
    ui.horizontal(|ui| {
        if loading {
            ui.spinner();
        }
        ui.button(text)
    }).inner
}