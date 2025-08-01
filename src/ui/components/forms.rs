use eframe::egui;
use egui::Vec2;

pub struct FormField<'a> {
    pub label: &'a str,
    pub value: &'a mut String,
    pub password: bool,
    pub placeholder: Option<&'a str>,
    pub validation_error: Option<&'a str>,
}

impl<'a> FormField<'a> {
    pub fn new(label: &'a str, value: &'a mut String) -> Self {
        Self {
            label,
            value,
            password: false,
            placeholder: None,
            validation_error: None,
        }
    }
    
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }
    
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
    
    pub fn validation_error(mut self, error: &'a str) -> Self {
        self.validation_error = Some(error);
        self
    }
    
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.label(self.label);
        
        let mut text_edit = egui::TextEdit::singleline(self.value);
        
        if self.password {
            text_edit = text_edit.password(true);
        }
        
        if let Some(placeholder) = self.placeholder {
            text_edit = text_edit.hint_text(placeholder);
        }
        
        let response = ui.add(text_edit);
        
        if let Some(error) = self.validation_error {
            ui.colored_label(egui::Color32::RED, error);
        }
        
        ui.add_space(10.0);
        response
    }
}

pub struct ComboField<'a, T> {
    pub label: &'a str,
    pub selected: &'a mut T,
    pub options: &'a [(T, &'a str)],
}

impl<'a, T: PartialEq + Clone> ComboField<'a, T> {
    pub fn new(label: &'a str, selected: &'a mut T, options: &'a [(T, &'a str)]) -> Self {
        Self {
            label,
            selected,
            options,
        }
    }
    
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.label(self.label);
        
        let selected_text = self.options
            .iter()
            .find(|(value, _)| value == self.selected)
            .map(|(_, text)| *text)
            .unwrap_or("Unknown");
        
        egui::ComboBox::from_label("")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for (value, text) in self.options {
                    ui.selectable_value(self.selected, value.clone(), *text);
                }
            });
        
        ui.add_space(10.0);
    }
}

pub fn form_section<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::group(ui.style())
        .inner_margin(20.0)
        .show(ui, |ui| {
            ui.set_max_width(max_width);
            content(ui)
        }).inner
}