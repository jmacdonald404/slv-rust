# Integration Plan: Performance, Scaling, and Networking

This document outlines a unified integration plan for `slv-rust`, combining the goals of three key documents:
1.  `perf.md`: The foundational high-performance architecture.
2.  `main_plan.md`: The detailed plan for implementing the networking layer.
3.  `Issue #7`: The proposal for dynamic performance scaling.

The guiding principle is **Performance by Default, Scalable by Design**. The core architecture will be built for high performance, while every component will be designed with hooks for dynamic adjustment based on hardware and user preference.

---

## Phase 1: Foundational Architecture & Configuration

This phase focuses on building the "steel frame" of the application and the systems that will control its behavior.

*   **Objective:** Implement the core concurrency model, the initial networking layer, and the complete performance configuration system.
*   **Related Docs:** `perf.md` (Part I), `main_plan.md` (Phase 1 & 2), `Issue #7` (Implementation).

### **Key Steps:**

1.  **Implement Core Concurrency Model:**
    *   Establish the Data-Oriented Design (DOD) principles for core data structures.
    *   Implement the job-based concurrency model using a global thread pool (e.g., `rayon`).
    *   Implement the async I/O pool (e.g., `tokio`) for networking and disk access.

2.  **Implement Protocol Definition & Code Generation:**
    *   Complete **Phase 1 (Protocol Parser)** and **Phase 2 (Code Generation)** from `main_plan.md`.
    *   This provides the raw, auto-generated message structs and codecs necessary for network communication.

3.  **Create the Performance Configuration System:**
    *   In `src/config/settings.rs`, define the data structures for all performance and scaling settings. This includes:
        *   An enum for `PerformanceProfile { Low, Balanced, High, Custom }`.
        *   A `PerformanceSettings` struct containing granular controls for rendering (draw distance, texture quality), memory (cache sizes), and networking (packet aggregation).
    *   Implement logic to load these settings from a configuration file.

4.  **Initial Hardware Detection:**
    *   Implement a one-time hardware detection routine on first launch.
    *   Use `wgpu::Adapter::get_info()` to inspect GPU capabilities and available RAM.
    *   Based on the detected hardware, select and save a default `PerformanceProfile` (`Low`, `Balanced`, or `High`).

---

## Phase 2: Core Rendering & Network Connection

This phase brings the world to life by implementing the core rendering pipeline and establishing a connection to the simulator.

*   **Objective:** Render a basic scene and handle the network handshake and state management, with rendering quality tied to the new configuration system.
*   **Related Docs:** `perf.md` (Part II), `main_plan.md` (Phase 3), `Issue #7` (Scaling Areas).

### **Key Steps:**

1.  **Implement Core Rendering Pipeline:**
    *   Integrate `wgpu` as the rendering backend.
    *   Implement Hierarchical-Z Buffer (HZB) for GPU-driven culling.
    *   Implement Clustered Forward Shading to handle dynamic lights efficiently.

2.  **Integrate Dynamic Rendering Settings:**
    *   Connect the `PerformanceSettings` to the renderer.
    *   The renderer will read the active profile to determine:
        *   **Shader Quality:** Use simplified shaders for `Low` profile.
        *   **Draw Distance:** Adjust camera far plane and culling distance.
        *   **Texture Filtering:** Use lower-quality texture filtering (e.g., bilinear) on `Low`.

3.  **Implement Connection Management:**
    *   Complete **Phase 3 (Connection & State Management)** from `main_plan.md`.
    *   The `Circuit` and `UdpTransport` modules will be refactored to use the auto-generated message code.
    *   The system will be capable of completing the region handshake.

4.  **Integrate Dynamic Network Settings:**
    *   Connect the `PerformanceSettings` to the networking layer.
    *   The `Low` profile might enable more aggressive packet aggregation or other bandwidth-saving measures.

---

## Phase 3: Advanced Asset Handling & Application Integration

This phase focuses on tackling the "content firehose" and enabling the application to respond to network messages.

*   **Objective:** Implement robust systems for handling unbounded, user-generated content and create the API for the rest of the application to interact with the network.
*   **Related Docs:** `perf.md` (Part III), `main_plan.md` (Phase 4), `Issue #7` (Scaling Areas).

### **Key Steps:**

1.  **Implement Advanced Asset Systems:**
    *   **Virtual Texturing (VT):** Implement a VT subsystem to manage texture memory.
    *   **Mesh Simplification:** Create the on-the-fly, background mesh decimator.
    *   **Impostor Generation:** Build the asynchronous impostor generation system.

2.  **Integrate Dynamic Asset & Memory Settings:**
    *   Connect the `PerformanceSettings` to the new asset systems:
        *   **Memory:** The VT cache size (VRAM and RAM) will be determined by the profile.
        *   **Asset Processing:** The aggressiveness of mesh simplification and the distance at which impostors are used will be controlled by the profile.

3.  **Implement Application Message Handling:**
    *   Complete **Phase 4 (Application Integration)** from `main_plan.md`.
    *   Define the message-passing API (channels) between the network layer and other systems (world, UI).
    *   Systems can now subscribe to specific network messages (e.g., `ObjectUpdate`) and react to them.

---

## Phase 4: UI, Adaptive Behavior & Testing

The final phase focuses on user-facing features, automated performance adjustments, and ensuring correctness.

*   **Objective:** Expose performance settings to the user, implement a system for runtime adaptation, and validate the entire stack with comprehensive tests.
*   **Related Docs:** `perf.md` (Part V), `main_plan.md` (Phase 5), `Issue #7` (Configuration, Adaptive Behavior).

### **Key Steps:**

1.  **Build Performance UI:**
    *   Create the "Preferences" window in the UI.
    *   Allow users to select a `PerformanceProfile` or enter a `Custom` mode to tweak individual settings.

2.  **Implement Adaptive Scaling (Optional/Toggleable):**
    *   Create a runtime monitoring system that tracks key metrics (FPS, memory pressure).
    *   If enabled, this system can automatically adjust the performance profile down (e.g., from `High` to `Balanced`) if performance targets are not met for a sustained period.

3.  **Implement Comprehensive Testing:**
    *   Complete **Phase 5 (Testing Strategy)** from `main_plan.md`.
    *   Create unit tests for protocol parsing and message codecs.
    *   Develop integration tests for the connection lifecycle.
    *   Establish performance benchmarks for rendering and asset processing under each of the performance profiles to validate the scaling implementation.
