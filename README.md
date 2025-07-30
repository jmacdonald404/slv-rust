# slv-rust: a SecondLife Viewer - Rust Implementation

(this is very WIP and until a stable release candidate is defined, ie: v1.x.x^, all of the codebase should be considered temporary and prone to change. minimal investigation of issues and support will be given until such a time.)

A modern SecondLife viewer implementation built with Rust, focusing on performance, safety, and modularity.

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

```
src/
├── main.rs                 # Application entry point
├── lib.rs                  # Library root
├── config/                 # Configuration management
│   ├── mod.rs
│   └── settings.rs
├── networking/             # Network communication
│   ├── mod.rs
│   ├── transport.rs        # UDP transport layer
│   ├── circuit.rs          # Circuit management
│   ├── protocol/           # SL protocol implementation
│   │   ├── mod.rs
│   │   ├── messages.rs     # Message definitions
│   │   └── codecs.rs       # Serialization/deserialization
│   └── session.rs          # Session management
├── rendering/              # Graphics and rendering
│   ├── mod.rs
│   ├── engine.rs           # Main rendering engine
│   ├── scene/              # Scene management
│   │   ├── mod.rs
│   │   ├── graph.rs        # Scene graph
│   │   └── culling.rs      # Frustum culling
│   ├── shaders/            # Shader programs
│   ├── materials.rs        # Material system
│   └── camera.rs           # Camera control
├── assets/                 # Asset management
│   ├── mod.rs
│   ├── manager.rs          # Asset loading/caching
│   ├── texture.rs          # Texture processing
│   ├── mesh.rs             # Mesh loading
│   └── cache.rs            # Asset caching
├── world/                  # Virtual world systems
│   ├── mod.rs
│   ├── avatar.rs           # Avatar system
│   ├── objects.rs          # Object management
│   ├── terrain.rs          # Terrain rendering
│   └── physics.rs          # Physics integration
├── ui/                     # User interface
│   ├── mod.rs
│   ├── main_window.rs      # Main application window
│   ├── inventory.rs        # Inventory management
│   ├── chat.rs             # Chat interface
│   └── preferences.rs      # Settings UI
└── utils/                  # Utility modules
    ├── mod.rs
    ├── math.rs             # Mathematical utilities
    └── logging.rs          # Logging setup
```

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

## Architectural Decision Records

We use Architecture Decision Records (ADRs) to document important architectural decisions. You can find them in the [`docs/adr`](docs/adr) directory.

-   [ADR-0001: Technology Stack Selection](docs/adr/0001-technology-stack-selection.md)
-   [ADR-0002: Networking Protocol Choice](docs/adr/0002-networking-protocol-choice.md)

## Development Roadmap

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
