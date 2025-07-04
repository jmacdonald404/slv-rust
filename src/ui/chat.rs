// TODO: Integrate chat UI with egui
// TODO: Implement chat window, input box, and message display

use egui::Context;
use std::collections::VecDeque;

pub fn show_chat_panel(ctx: &Context, chat_input: &mut String, chat_messages: &mut VecDeque<String>) {
    egui::Window::new("Chat").show(ctx, |ui| {
        ui.label("Chat messages:");
        for msg in chat_messages.iter() {
            ui.label(msg);
        }
        ui.separator();
        let send = ui.text_edit_singleline(chat_input).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if send && !chat_input.trim().is_empty() {
            chat_messages.push_back(chat_input.clone());
            if chat_messages.len() > 10 {
                chat_messages.pop_front();
            }
            chat_input.clear();
        }
    });
}
