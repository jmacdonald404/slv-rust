# v0.2.0-alpha Release Notes

## Major Features Completed

### Basic UI Framework
- Integrated egui for immediate-mode GUI in `src/ui/`.
- Implemented main window, chat, inventory, and preferences panels with basic state and interactivity.
- State is passed to each panel and the UI is ready for further expansion.
  - See: `src/ui/mod.rs`, `src/ui/main_window.rs`, `src/ui/chat.rs`, `src/ui/inventory.rs`, `src/ui/preferences.rs`

### Avatar System
- Implemented basic Avatar struct with id, name, mesh_id, texture_id, and pose.
- Methods for mesh/texture assignment, animation update, and appearance loading.
- Ready for future animation and rendering integration.
  - See: `src/world/avatar.rs` (lines 8-49)

### Object Rendering
- Implemented Object struct with id, transform, mesh_id, material_id.
- SceneGraph supports adding, removing, updating, and (stub) rendering of objects.
- Integrates with asset system by id and is ready for draw call integration.
  - See: `src/rendering/scene/graph.rs` (lines 1-49)

### Terrain System
- Implemented Terrain struct with mesh_id and LOD.
- Methods for mesh generation, rendering, and editing (all stubbed for future logic).
- Integrates with asset/scene management by mesh_id.
  - See: `src/world/terrain.rs` (lines 7-38)

### Basic Physics
- Implemented PhysicsWorld and PhysicsObject structs with object list, position/velocity/mass properties.
- Object registration, update (moves objects by velocity), and stubbed collision handling.
- Ready for future rapier3d integration.
  - See: `src/world/physics.rs` (lines 7-44)

---

All major v0.2.0 roadmap features are now in place as a foundation for further development. Advanced features, deeper integration, and polish are the next steps. 🎉