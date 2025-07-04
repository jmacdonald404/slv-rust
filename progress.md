## Feature: Basic Asset Loading System Implemented

**Description:**
The core asset loading system has been implemented, focusing on textures. This includes defining generic asset loading mechanisms, a caching system, and integrating texture loading directly into the rendering pipeline.

**Details of Implementation:**
- Defined `AssetLoader` trait and `AssetCache` for generic asset management.
- Implemented `TextureLoader` to load image files into `wgpu::Texture` objects.
- Integrated the new asset loading and caching into the `RenderEngine`, replacing hardcoded texture loading.
- Updated `winit` and `wgpu` API usage to resolve compilation errors and ensure compatibility.
- Added vertex and index buffers for basic rendering of a textured primitive.

**Remaining Work:**

    1. Implement other asset types (Mesh, Material, Shader) and their loaders.
       * Define asset types for Mesh, Material, and Shader. (DONE)
       * Create AssetLoader implementations for Mesh, Material, and Shader. (DONE)
   2. Integrate mesh loading into the rendering pipeline:
       * Load a default mesh using the new asset system. (DONE)
       * Pass the loaded mesh to the render pipeline. (DONE)
   3. Develop a scene graph and object management system.
       * Create a SceneGraph structure to manage hierarchical transformations. (DONE)
       * Implement methods to add, remove, and update objects in the scene. (DONE)
   4. Basic lighting:
       * Add a simple directional light or ambient light. (DONE)
       * Update shaders to incorporate lighting. (DONE)
   5. Error handling and logging:
       * Improve error handling throughout the engine. (DONE)
       * Integrate with the tracing crate for better logging. (DONE)
   6. Refactor `RenderEngine` for better modularity:
       * Break down RenderEngine into smaller, more manageable components (e.g., Renderer, ResourceManager).
   7. Clean up and optimize:
       * Remove unused code, optimize performance.
       * Ensure proper resource management (e.g., dropping WGPU resources).
       * General cleanup and optimization.

## Progress Update (as of 2024-06-09)

### File/Line Pointers for Implemented Features

- AssetLoader trait and AssetCache: `src/assets/manager.rs`, `src/assets/cache.rs`
- TextureLoader: `src/assets/texture.rs`
- RenderEngine asset integration: `src/rendering/engine.rs` (lines 1–330)
- Mesh asset type/loader: `src/assets/mesh.rs`
- SceneGraph: `src/rendering/scene/graph.rs` (lines 1–27)
- Object struct: `src/rendering/scene/mod.rs` (lines 4–13)
- Light struct/uniform: `src/rendering/light.rs` (lines 1–28)
- Lighting: Lighting is now integrated in the render loop and shader. The render loop updates the light uniform buffer and the mesh is rendered with lighting applied. (See `src/rendering/engine.rs` lines ~330-420, `src/rendering/shaders/shader.wgsl` lines 1-31)
- Error Handling/Logging: Add robust error handling and integrate `tracing` throughout. (All modules, especially asset loading and rendering)

### TODOs

- [ ] **Refactor RenderEngine**: Break into smaller modules for modularity. (See `src/rendering/engine.rs`)
- [ ] **Cleanup/Optimization**: Remove unused code, optimize, ensure proper resource management. (All modules)