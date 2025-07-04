// TODO: Integrate inventory UI with egui
// TODO: Implement inventory window and item management

use egui::Context;

pub fn show_inventory_panel(ctx: &Context, inventory_items: &[String]) {
    egui::Window::new("Inventory").show(ctx, |ui| {
        ui.label("Inventory items:");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in inventory_items.iter() {
                ui.label(item);
                // TODO: Add selection, drag-and-drop, context menu, etc.
            }
        });
    });
}
