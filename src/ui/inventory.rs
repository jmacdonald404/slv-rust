// TODO: Integrate inventory UI with egui
// TODO: Implement inventory window and item management

use eframe::egui::Context;

// pub fn show_inventory_panel(ctx: &eframe::egui::Context, inventory_items: &[String]) {
//     eframe::egui::Window::new("Inventory").show(ctx, |ui| {
//         ui.label("Inventory items:");
//         eframe::egui::ScrollArea::vertical().show(ui, |ui| {
//             for item in inventory_items.iter() {
//                 ui.label(item);
//                 // TODO: Add selection, drag-and-drop, context menu, etc.
//             }
//         });
//     });
// }
