# ADR-0001: Technology Stack Selection

**Date**: 2025-07-30

**Status**: Accepted

## Context

The `slv-rust` project aims to be a modern, performant, and cross-platform SecondLife viewer. The choice of core technologies for networking, rendering, and UI is critical to achieving these goals. The decision must consider the trade-offs between performance, safety, community support, and development velocity.

The primary requirements are:
-   **Networking**: High-performance, asynchronous I/O to handle the real-time nature of SecondLife's UDP-based protocol.
-   **Rendering**: A modern graphics API that can target multiple platforms (Windows, macOS, Linux) and provides good performance.
-   **UI**: A flexible and easy-to-use UI framework for building the complex user interface of a SecondLife viewer.

## Decision

We have chosen the following core technology stack:

-   **Asynchronous Runtime**: `tokio`
-   **Rendering Engine**: `wgpu`
-   **UI Framework**: `eframe` (which uses `egui`)

### Rationale

#### `tokio` for Asynchronous Networking

-   **Performance and Scalability**: `tokio` is a highly-optimized, production-ready asynchronous runtime for Rust. Its work-stealing scheduler is well-suited for handling a large number of concurrent network connections and I/O operations, which is essential for a SecondLife viewer.
-   **Ecosystem**: `tokio` has a rich ecosystem of libraries, including `hyper` for HTTP, `tonic` for gRPC, and `tokio-tungstenite` for WebSockets. While our primary protocol is UDP, this ecosystem is valuable for auxiliary services like login and asset fetching.
-   **Community and Support**: `tokio` is the de facto standard for asynchronous programming in Rust, with a large community and excellent documentation.

#### `wgpu` for Rendering

-   **Cross-Platform**: `wgpu` is a web-first graphics API that is also a native Rust library. It provides a safe and idiomatic Rust API that abstracts over modern graphics backends like Vulkan, Metal, DirectX 12, and OpenGL. This allows `slv-rust` to be truly cross-platform.
-   **Safety**: `wgpu` is designed with safety in mind, reducing the risk of GPU-related crashes and undefined behavior. This aligns with the project's goal of building a stable and reliable viewer.
-   **Modern API**: `wgpu` provides a modern API that is closer to Vulkan and Metal than OpenGL. This allows for better performance and more control over the GPU.

#### `eframe` for UI

-   **Immediate Mode GUI**: `eframe` uses `egui`, an immediate-mode GUI library. This approach simplifies UI development by allowing the UI to be defined declaratively as part of the application's state. This is in contrast to retained-mode GUI frameworks, which can be more complex to manage.
-   **Integration with `wgpu`**: `eframe` has excellent integration with `wgpu` through the `egui-wgpu` crate. This makes it easy to render the UI on top of the 3D scene.
-   **Ease of Use**: `egui` is known for its simplicity and ease of use, which will allow for faster iteration on the UI.

## Consequences

### Positive

-   We have a solid foundation for building a high-performance, cross-platform viewer.
-   The chosen libraries are well-maintained and have strong community support.
-   The combination of `tokio`, `wgpu`, and `eframe` provides a cohesive and powerful stack for this type of application.

### Negative

-   `wgpu` is a lower-level API than a full-featured game engine like Bevy or Fyrox. This means we will need to implement more of the rendering pipeline ourselves (e.g., scene graph, culling).
-   Immediate-mode GUIs can have performance challenges with very complex UIs, but `egui` is highly optimized and this is not expected to be a major issue for our use case.

## Alternatives Considered

-   **`async-std`**: Another popular async runtime. While excellent, `tokio` has a larger ecosystem and is more widely used in the community.
-   **OpenGL**: Using a library like `glium` or `glow`. While OpenGL is more widely known, `wgpu` provides a more modern and safer API.
-   **Bevy Engine**: Bevy is a full-featured game engine that includes rendering, networking, and UI. While it is a powerful option, it also brings a lot of its own opinions and abstractions. By choosing `tokio`, `wgpu`, and `eframe` separately, we have more control over the architecture of our application.
