# slv-rust: Architecture

This document outlines the software architecture for `slv-rust`, a modern SecondLife viewer. It is designed to be a high-performance, modular, and maintainable client, built on the principles of Data-Oriented Design.

## Mission Critical Architecture Principles

**🔥 SEPARATION OF CONCERNS**: Every component is isolated in individual files with single responsibilities. This ensures maintainability, testability, and contributor accessibility. No file should contain multiple unrelated functionalities.

**🔥 SECONDLIFE PROTOCOL COMPLIANCE**: All networking implementations must strictly adhere to SecondLife's official protocols. Reference implementations in `homunculus/` (TypeScript) and `hippolyzer/` (Python), plus `message_template.msg`, are our canonical sources. Protocol violations result in connection failures.

**🔥 DEVELOPMENT JOURNAL**: All architectural decisions, roadblocks, and recurring issues must be documented in `DEVELOPMENT_JOURNAL.md`. This includes performance bottlenecks, protocol quirks, dependency conflicts, and their resolutions. The journal serves as institutional knowledge for future development.

**🦀 RUST ADVANTAGES**: Leverage Rust's type system for compile-time correctness, zero-cost abstractions for performance, and memory safety for reliability. Prefer `Result<T, E>` over panics, use `#[derive]` for boilerplate reduction.

## 1. Core Philosophy: Data-Oriented Design (DOD)

The entire architecture is built upon a Data-Oriented Design philosophy. This is a fundamental shift away from traditional Object-Oriented Programming (OOP).

*   **Why DOD?** Modern CPU performance is dominated by memory latency. DOD structures the program around the data and its transformations, organizing data in contiguous arrays to maximize CPU cache hits. This is the key to unlocking scalable performance, especially in a concurrent environment.
*   **Implementation:** Instead of an array of `Avatar` objects, we have separate, contiguous arrays for each component: a `positions` array, a `velocities` array, a `mesh_references` array, etc. Systems operate on these arrays, leading to highly predictable memory access patterns.

## 2. Concurrency Model: A Job-Based System

We use a multi-threaded architecture to separate concerns and maximize throughput. The model is divided into two main pools:

*   **Async I/O Pool (`tokio`)**: Manages all I/O-bound tasks, primarily networking (UDP communication with the simulator) and disk access (asset streaming). By using an async runtime, we can handle thousands of concurrent I/O operations without blocking expensive OS threads.
*   **Compute Job Pool (`rayon`)**: Manages all CPU-bound tasks. Work is broken down into small, independent "jobs" (e.g., "decompress texture," "simplify mesh," "cull objects") which are dynamically scheduled across all available CPU cores. This provides automatic load balancing and scales seamlessly with CPU core count.

| System / Thread Pool | Primary Responsibilities                               | Key Data (Write)                                     | Synchronization          |
|----------------------|--------------------------------------------------------|------------------------------------------------------|--------------------------|
| **Main Thread**      | User input, OS window events, final frame coordination | Command Queues for other systems                     | Event Loop, Channels     |
| **Async I/O Pool**   | Network communication, disk I/O (asset loading)        | Raw Asset Data Buffers                               | Async Tasks, Futures     |
| **Compute Job Pool** | Asset decompression, mesh parsing, scene updates, culling, physics, render command generation | Asset Caches, Updated Transforms, Visibility Lists | Lock-free Queues, Parallel Iterators |

## 3. Networking Layer: SecondLife Protocol Compliance

The networking layer is designed as a self-contained, modular component that communicates with the rest of the application via asynchronous channels. **CRITICAL**: All implementations must reference `homunculus/` and `hippolyzer/` as authoritative examples. For a detailed technical analysis of the Second Life protocol, refer to `secondlife-protocol-technical-analysis.md`.

### 3.1 File Organization (Separation of Concerns)
```
networking/
├── mod.rs              # Module exports only
├── transport.rs        # UDP transport (one responsibility)
├── circuit.rs          # Circuit management (one responsibility) 
├── auth/
│   ├── mod.rs          # Module exports only
│   ├── login.rs        # Authentication service
│   ├── session.rs      # Session state management
│   ├── grid.rs         # Grid configuration
│   └── xmlrpc.rs       # XML-RPC client (matches homunculus/)
├── protocol/
│   ├── mod.rs          # Module exports only
│   ├── messages.rs     # Message definitions (from message_template.msg)
│   └── codecs.rs       # Serialization (matches protocol spec)
└── handlers/
    ├── mod.rs          # Module exports only
    └── [handler].rs    # Individual message handlers
```

### 3.2 Implementation Phases
*   **Phase 1: Protocol Parsing:** A dedicated parser reads the `message_template.msg` file, making it the single source of truth for the SecondLife protocol.
*   **Phase 2: Code Generation:** A `build.rs` script uses the parsed data to automatically generate all Rust message structs and serialization/deserialization codecs. This eliminates manual, error-prone implementation.
*   **Phase 3: Connection Management:** A `Circuit` manager handles the UDP connection state, reliability (ACKs, retransmissions), and the handshake sequence, using the auto-generated message code.
*   **Phase 4: Application Integration:** The networking layer communicates with the main application via `tokio::sync::mpsc` channels:
    *   **`NetworkCommand` Channel (App -> Net):** The application sends high-level commands (e.g., `SendChat`) to the networking layer.
    *   **Event Channels (Net -> App):** The networking layer dispatches events (e.g., `ChatEvent`, `ObjectUpdateEvent`) to the relevant application modules.
*   **Phase 5: Testing:** A multi-layered testing strategy ensures correctness, from unit tests on the parser to end-to-end tests against the live grid.

### 3.3 Protocol Reference Requirements
- **XML-RPC Authentication**: Must match `homunculus/packages/homunculus-core/src/network/authenticator.ts`
- **Message Format**: Must align with `message_template.msg` specifications
- **Circuit Handshake**: Must follow examples in `hippolyzer/` implementation
- **Password Hashing**: MD5 with 16-character truncation as per SecondLife spec

## 4. Rendering Pipeline: A GPU-Driven Approach

The rendering pipeline is designed to be "GPU-driven," minimizing CPU bottlenecks and maximizing the GPU's parallel processing power.

*   **Graphics API (`wgpu`):** We use `wgpu` as a safe, cross-platform abstraction over modern graphics APIs (Vulkan, Metal, DX12). This gives us the performance benefits of these APIs without the extreme complexity of using them directly.
*   **GPU-Driven Culling (HZB):** We use Hierarchical-Z Buffer (HZB) culling. A depth pre-pass and a compute shader are used to determine object visibility entirely on the GPU, eliminating CPU-GPU round trips. The result is a compact list of visible objects that can be rendered with a single indirect draw call.
*   **Clustered Forward Shading (Forward+):** To handle scenes with many dynamic lights, we divide the view frustum into a 3D grid of "clusters." A compute shader assigns lights to these clusters. The pixel shader then only performs lighting calculations for the lights within its cluster, decoupling shading cost from the total number of lights in the scene.

## 5. Asset Handling

The asset system is designed to handle the unpredictable stream of user-generated content without stalling.

*   **Virtual Texturing (VT):** To handle the massive amount of texture data, we use a Virtual Texturing system. All textures are treated as part of a single, enormous virtual texture, which is tiled and stored on disk. A feedback pass determines which tiles are needed, and they are streamed into a fixed-size GPU cache on demand. This decouples the scene's visual complexity from VRAM limitations.
*   **On-the-Fly Mesh Simplification:** For complex meshes without server-provided Levels of Detail (LoDs), a background job uses the Quadric Edge Collapse Decimation (QED) algorithm to generate simplified versions. The renderer then selects the appropriate LoD based on distance.
*   **Asynchronous Impostor Generation:** For very distant objects, a background process renders the full 3D mesh to a texture (an "impostor") which is then used to replace the real geometry with a simple, camera-facing quad.

This architecture is designed to be a high-performance, scalable, and maintainable foundation for a modern virtual world viewer.
