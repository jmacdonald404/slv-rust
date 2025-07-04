# v0.1.1-alpha Release Notes

## Core Features Implemented

### File/Line Pointers for Implemented Features

---

## TODO: Next Steps Toward v0.2.0

- [ ] **Basic UI Framework**: Integrate an immediate-mode GUI (egui) for basic HUD and settings panels. Set up UI event loop and rendering in `src/ui/`.
  - Added comment stubs for egui integration and UI event loop in `src/ui/mod.rs` (lines 1-10).
  - Added stubs for main window, chat, inventory, and preferences panels in `src/ui/main_window.rs`, `src/ui/chat.rs`, `src/ui/inventory.rs`, and `src/ui/preferences.rs` (lines 1-3 in each).
  - These stubs outline where egui context, event handling, and UI panels will be implemented for the HUD and settings.
  - Implemented egui_wgpu painting in `src/ui/mod.rs` (see UiRenderer struct and run_ui_frame, lines 13-80).
  - All major panels now have basic state and interactivity: chat (message input/display), inventory (item list), preferences (sound toggle, volume slider). State is passed to each panel. (See `src/ui/mod.rs`, `src/ui/chat.rs`, `src/ui/inventory.rs`, `src/ui/preferences.rs`)
- [ ] **Avatar System**: Implement basic avatar representation and appearance loading in `src/world/avatar.rs`. Support for mesh/texture assignment and simple animation stubs.
- [ ] **Object Rendering**: Add support for rendering multiple scene objects using the scene graph (`src/rendering/scene/graph.rs`). Implement object transforms and per-object material/mesh assignment.
- [ ] **Terrain System**: Implement terrain mesh generation and rendering in `src/world/terrain.rs`. Integrate with asset and scene management.
- [ ] **Basic Physics**: Integrate a physics engine (e.g., rapier3d) for simple collision detection and rigid body dynamics in `src/world/physics.rs`.