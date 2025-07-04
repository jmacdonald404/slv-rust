# slv-rust: a SecondLife Viewer - Rust Implementation

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
# Networking
tokio = { version = "1.38.0", features = ["full"] }
quinn = "0.11.0"
bytes = "1.6.0"

# Rendering
wgpu = "0.20.1"  # Cross-platform graphics
winit = "0.30.3"  # Window management
glam = "0.28.0"   # Linear algebra
image = "0.25.1"  # Image processing

# Asset Processing
jpeg2k-sandboxed = "0.9.1"  # JPEG2000 decoder
collada = "0.16.0"   # DAE mesh format
hound = "3.5.1"     # Audio processing

# UI Framework
egui = "0.28.1"     # Immediate mode GUI
egui-wgpu = "0.28.1"
egui-winit = "0.28.1"

# Serialization
serde = { version = "1.0.203", features = ["derive"] }
bincode = "1.3.3"   # Binary serialization
uuid = { version = "1.9.1", features = ["v4"] }

# Utilities
tracing = "0.1.40"   # Logging
tracing-subscriber = "0.3.18"
config = "0.14.0"   # Configuration management
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

## Development Roadmap

### Phase 1: Core Infrastructure
- [x] Basic networking layer
- [ ] Fundamental rendering pipeline
- [ ] Asset loading system
- [ ] Basic UI framework

### Phase 2: World Integration
- [ ] Avatar system
- [ ] Object rendering
- [ ] Terrain system
- [ ] Basic physics

### Phase 3: Feature Completeness
- [ ] LSL scripting support
- [ ] Advanced lighting
- [ ] Audio system
- [ ] Chat and communication

### Phase 4: Optimization
- [ ] Performance profiling
- [ ] Memory optimization
- [ ] Network optimization
- [ ] Rendering optimization

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and coding standards.

## License

This project is licensed under the MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- SecondLife protocol documentation
- Open source viewer projects (Firestorm, Singularity)
- Rust graphics and networking communities
