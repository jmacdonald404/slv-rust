use eframe::egui;
use egui::Color32;

pub enum StatusLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl StatusLevel {
    fn color(&self) -> Color32 {
        match self {
            StatusLevel::Info => Color32::from_rgb(23, 162, 184),
            StatusLevel::Success => Color32::from_rgb(40, 167, 69),
            StatusLevel::Warning => Color32::from_rgb(255, 193, 7),
            StatusLevel::Error => Color32::from_rgb(220, 53, 69),
        }
    }
    
    fn icon(&self) -> &'static str {
        match self {
            StatusLevel::Info => "ⓘ",
            StatusLevel::Success => "✓",
            StatusLevel::Warning => "⚠",
            StatusLevel::Error => "✗",
        }
    }
}

pub struct StatusMessage {
    pub level: StatusLevel,
    pub message: String,
    pub show_icon: bool,
}

impl StatusMessage {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Info,
            message: message.into(),
            show_icon: true,
        }
    }
    
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Success,
            message: message.into(),
            show_icon: true,
        }
    }
    
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Warning,
            message: message.into(),
            show_icon: true,
        }
    }
    
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: StatusLevel::Error,
            message: message.into(),
            show_icon: true,
        }
    }
    
    pub fn without_icon(mut self) -> Self {
        self.show_icon = false;
        self
    }
    
    pub fn show(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.show_icon {
                ui.colored_label(self.level.color(), self.level.icon());
            }
            ui.colored_label(self.level.color(), &self.message);
        });
    }
}

pub fn show_progress_bar(ui: &mut egui::Ui, progress: f32, text: Option<&str>) {
    let progress_bar = egui::ProgressBar::new(progress)
        .show_percentage();
    
    ui.add(progress_bar);
    
    if let Some(text) = text {
        ui.label(text);
    }
}

pub fn show_spinner_with_text(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(text);
    });
}