# slv-rust: a SecondLife Viewer - Rust Implementation

(this is very WIP and until a stable release candidate is defined, ie: v1.x.x^, all of the codebase should be considered temporary and prone to change. minimal investigation of issues and support will be given until such a time.)

A modern SecondLife viewer implementation built with Rust, focusing on performance, safety, and modularity.

## Mission Critical Principles

**🔥 SEPARATION OF CONCERNS**: This project maintains strict separation of concerns with individual files per component. Each module must be well-documented, self-contained, and easy to maintain. This is non-negotiable for project maintainability and contributor onboarding.

**🔥 SECONDLIFE PROTOCOL COMPLIANCE**: All networking code must strictly respect SecondLife server protocols. Reference implementations in `homunculus/` and `hippolyzer/` directories, along with `message_template.msg`, serve as our authoritative sources. Protocol deviations can result in connection failures or grid bans.

**🔥 DEVELOPMENT JOURNAL**: Maintain a comprehensive journal of roadblocks, recurring bugs, and development bottlenecks in `DEVELOPMENT_JOURNAL.md`. This serves as our source of truth for documenting quirks, solutions, and lessons learned. Every significant issue must be documented with context, attempted solutions, and final resolution.

**🦀 RUST STRENGTHS**: Leverage Rust's type system, memory safety, and zero-cost abstractions. Prefer compile-time guarantees over runtime checks wherever possible.

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Application   │    │   Networking    │    │   Rendering     │
│     Layer       │◄──►│     Layer       │◄──►│     Engine      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│      UI/UX      │    │   Protocol      │    │   Asset/Scene   │
│    Management   │    │   Handlers      │    │   Management    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Core Components

### 1. Networking Layer
- **UDP Transport**: Low-latency communication with SecondLife servers
- **Message Serialization**: Binary protocol handling for SL messages
- **Connection Management**: Circuit establishment and maintenance
- **Packet Reliability**: Automatic retransmission and ordering

### 2. Rendering Engine
- **Graphics Backend**: Vulkan-first with OpenGL fallback
- **Scene Graph**: Hierarchical 3D scene management
- **LOD System**: Level-of-detail for meshes and textures
- **Lighting**: Dynamic lighting with shadow mapping
- **Water/Terrain**: Specialized rendering for SL environments

### 3. Asset Management
- **Texture Pipeline**: JPEG2000 decoding and GPU upload
- **Mesh Processing**: Collada DAE and native SL mesh formats
- **Animation System**: Skeletal animation with BVH support
- **Audio Engine**: 3D positional audio with streaming

### 4. Virtual World Systems
- **Avatar System**: Appearance, animations, and attachments
- **Physics Engine**: Collision detection and rigid body dynamics
- **Scripting Interface**: LSL script execution environment
- **Parcel Management**: Land rights and restrictions

## Technology Stack

### Core Dependencies
```toml
[dependencies]
# --- Networking ---
bytes = "1.10.1"
quinn = "0.11.8"
tokio = { version = "1.46.0", features = ["full", "fs"] }

# --- Rendering ---
bytemuck = { version = "1.23.1", features = ["derive"] }
cgmath = "0.18.0"
glam = "0.30.4"
image = "0.25.6"
wgpu = "24.0.5"
winit = "0.30.7"

# --- Asset Processing ---
collada = "0.16.0"
jpeg2k-sandboxed = "0.9.1"
hound = "3.5.1"

# --- UI Framework ---
eframe = "0.31"

# --- Serialization ---
bincode = "2.0.1"
serde = { version = "1.0.219", features = ["derive"] }
uuid = { version = "1.17.0", features = ["v4"] }

# --- Utilities ---
anyhow = "1.0"
async-trait = "0.1.88"
bitflags = "2.9.1"
config = "0.15.11"
crossbeam-channel = "0.5.15"
pollster = "0.4.0"
rand = "0.9.1"
regex = "1"
scraper = "0.23.1"
serde_json = "1.0.140"
thiserror = "2.0"
toml = "0.8"
tracing = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["env-filter"] }

# --- HTTP & XML ---
quick-xml = { version = "0.31", features = ["serialize"] }
reqwest = { version = "0.12", features = ["json", "gzip", "deflate"] }
roxmltree = "0.20"
```

### Optional Dependencies
```toml
[dependencies]
# Physics (choose one)
rapier3d = { version = "0.19.0", optional = true }

# Audio
rodio = { version = "0.18.1", optional = true }
cpal = { version = "0.15.3", optional = true }

# Compression
flate2 = { version = "1.0.30", optional = true }
lz4 = { version = "1.25.0", optional = true }
```

## Project Structure

**Each component follows strict separation of concerns - one responsibility per file.**

```
src/
├── main.rs                 # Application entry point
├── lib.rs                  # Library root
├── config/                 # Configuration management
│   ├── mod.rs              # Module exports only
│   └── settings.rs         # Settings loading/validation
├── networking/             # Network communication (SL protocol compliance)
│   ├── mod.rs              # Module exports only  
│   ├── transport.rs        # UDP transport layer
│   ├── circuit.rs          # Circuit management per SL spec
│   ├── auth/               # Authentication (XML-RPC per homunculus)
│   │   ├── mod.rs          # Module exports only
│   │   ├── login.rs        # Login service
│   │   ├── session.rs      # Session state management
│   │   └── xmlrpc.rs       # XML-RPC client implementation
│   ├── protocol/           # SL protocol implementation
│   │   ├── mod.rs          # Module exports only
│   │   ├── messages.rs     # Message definitions (from message_template.msg)
│   │   └── codecs.rs       # Serialization/deserialization
│   └── handlers/           # Message handlers
│       ├── mod.rs          # Module exports only
│       └── [handler].rs    # Individual message handlers
├── rendering/              # Graphics and rendering
│   ├── mod.rs              # Module exports only
│   ├── engine.rs           # Main rendering engine
│   ├── scene/              # Scene management
│   │   ├── mod.rs          # Module exports only
│   │   ├── graph.rs        # Scene graph management
│   │   └── culling.rs      # Frustum culling algorithms
│   ├── shaders/            # Shader programs (separated by purpose)
│   ├── materials.rs        # Material system
│   └── camera.rs           # Camera control
├── assets/                 # Asset management
│   ├── mod.rs              # Module exports only
│   ├── manager.rs          # Asset loading/caching coordinator
│   ├── texture.rs          # Texture processing (JPEG2000, etc)
│   ├── mesh.rs             # Mesh loading (Collada, SL formats)
│   └── cache.rs            # Asset caching strategies
├── world/                  # Virtual world systems
│   ├── mod.rs              # Module exports only
│   ├── avatar.rs           # Avatar system
│   ├── objects.rs          # Object management
│   ├── terrain.rs          # Terrain rendering
│   └── physics.rs          # Physics integration
├── ui/                     # User interface
│   ├── mod.rs              # Module exports only
│   ├── main_window.rs      # Main application window
│   ├── inventory.rs        # Inventory management
│   ├── chat.rs             # Chat interface
│   └── preferences.rs      # Settings UI
└── utils/                  # Utility modules
    ├── mod.rs              # Module exports only
    ├── math.rs             # Mathematical utilities
    └── logging.rs          # Logging setup
```

**Documentation Requirements**: Every `.rs` file must include comprehensive module-level documentation explaining its purpose, key types, and integration points.

## Key Technical Specifications

### Network Protocol
- **Transport**: UDP with custom reliability layer
- **Message Format**: Binary with Little Endian encoding
- **Circuit Management**: Automatic keep-alive and reconnection
- **Bandwidth**: Adaptive bandwidth allocation
- **Compression**: Zlib for large messages

### Rendering Pipeline
- **API**: wgpu (Vulkan/DirectX 12/Metal)
- **Shading**: PBR with custom SL material extensions
- **Textures**: JPEG2000 with progressive loading
- **Geometry**: Indexed triangle meshes with LOD
- **Lighting**: Forward+ rendering with clustered lights

### Asset System
- **Formats**: 
  - Textures: JPEG2000, TGA, PNG
  - Meshes: Collada DAE, SL native format
  - Audio: Ogg Vorbis, WAV
  - Animations: BVH, SL native
- **Caching**: LRU cache with configurable size limits
- **Streaming**: Progressive asset loading

### Performance Targets
- **Frame Rate**: 60 FPS minimum on mid-range hardware
- **Memory Usage**: <2GB RAM for typical usage
- **Startup Time**: <10 seconds to world entry
- **Network Latency**: <100ms response time

## Build Instructions

### Prerequisites
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install additional dependencies (Ubuntu/Debian)
sudo apt-get install build-essential cmake pkg-config libssl-dev

# Install additional dependencies (macOS)
brew install cmake pkg-config openssl

# Install additional dependencies (Windows)
# Install Visual Studio Build Tools 2019 or later
```

### Building
```bash
# Clone the repository
git clone git@github.com:jmacdonald404/slv-rust
cd slv-rust

# Build in release mode
cargo build --release

# Run with default configuration
cargo run --release

# Run with custom configuration
cargo run --release -- --config custom_config.toml
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test suite
cargo test networking

# Run with logging
RUST_LOG=debug cargo test
```

## Configuration

### Basic Configuration (`config.toml`)
```toml
[network]
grid_uri = "https://login.agni.lindenlab.com/cgi-bin/login.cgi"
max_bandwidth = 1500  # KB/s
timeout = 30          # seconds

[rendering]
graphics_api = "vulkan"  # vulkan, dx12, metal, opengl
vsync = true
max_fps = 60
render_distance = 256    # meters

[cache]
texture_cache_size = 1024  # MB
mesh_cache_size = 512      # MB
cache_directory = "cache"

[audio]
master_volume = 0.8
ui_volume = 0.7
environment_volume = 0.9
```

## Architecture and Planning

This project's architecture and development are guided by a set of core documents that outline our technical strategy, design principles, and implementation plan.

-   **[`ARCHITECTURE.md`](./ARCHITECTURE.md):** The canonical source for the project's software architecture, including our Data-Oriented Design philosophy, concurrency model, and the design of the rendering and networking pipelines.
-   **[`main_plan.md`](./main_plan.md):** The high-level implementation plan for the networking layer, broken down into five distinct phases.
-   **[`perf.md`](./perf.md):** A detailed expert report on the viability and implementation strategy for achieving a high-performance, Rust-based virtual world viewer. It covers the foundational architecture, rendering pipeline, and advanced asset handling strategies.
-   **[`docs/adr`](./docs/adr):** A collection of Architecture Decision Records (ADRs) for specific, important technical decisions.

### Phase 1: Core Infrastructure [v0.1.0-alpha]
- [x] Basic networking layer
- [x] Fundamental rendering pipeline
- [x] Asset loading system
- [x] Basic UI framework

### Phase 2: World Integration [v0.2.0-alpha]
- [x] Avatar system
- [x] Object rendering
- [x] Terrain system
- [x] Basic physics

### Phase 3: Stability and Testing [v0.3.0-alpha]
- [ ] Core UI
- [ ] Debugging/Release Build Flags
- [ ] Login/Logout & Base Integration with Secondlife
- [ ] Build Error and Warning Resolution

### Phase 4: Feature Completeness
- [ ] LSL scripting support
- [ ] Advanced lighting
- [ ] Audio system
- [ ] Chat and communication

### Phase 5: Optimization
- [ ] Performance profiling
- [ ] Memory optimization
- [ ] Network optimization
- [ ] Rendering optimization

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and coding standards. All contributors are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md).

For information on debugging the application, see [DEBUGGING.md](DEBUGGING.md).

## License

This project is licensed under the MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- SecondLife protocol documentation
- Open source viewer projects (Firestorm, Singularity)
- Rust graphics and networking communities
