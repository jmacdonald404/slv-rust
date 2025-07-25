# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

slv-rust is a Second Life viewer implementation in Rust, currently in v0.3.0-alpha development. The project follows the principle **"Performance by Default, Scalable by Design"** - building a high-performance virtual world client that dynamically adapts to hardware capabilities from low-end to high-end systems.

## Development Commands

**Build and Run:**
```bash
cargo build          # Build the project
cargo run            # Run the main application
cargo build --release # Release build for performance testing
```

**Testing:**
```bash
cargo test           # Run all tests
cargo test -- --nocapture # Run tests with output
```

**Development Tools:**
```bash
cargo check          # Fast syntax/type checking
cargo clippy         # Linting
cargo fmt            # Code formatting
```

## Architecture Overview

### Core Design Principles

**Performance by Default, Scalable by Design:** The architecture is built for high performance while providing dynamic adjustment hooks for different hardware configurations.

The codebase follows a **unified 4-phase implementation plan** detailed in `int.md`:

**Phase 1: Foundational Architecture & Configuration** 
- Core concurrency model (DOD + job-based + async I/O pools)
- Protocol definition parsing & automated code generation  
- Performance configuration system with hardware detection
- **Status:** In Progress (Protocol parsing ✅, Code generation ✅)

**Phase 2: Core Rendering & Network Connection**
- wgpu rendering pipeline with dynamic quality settings
- Connection management with performance-aware networking
- **Status:** Not Started

**Phase 3: Advanced Asset Handling & Application Integration** 
- Virtual texturing, mesh simplification, impostor generation
- Message-passing API between network and application layers
- **Status:** Not Started

**Phase 4: UI, Adaptive Behavior & Testing**
- Performance preferences UI and adaptive scaling system
- Comprehensive testing strategy
- **Status:** Not Started

### Key Architectural Components

**Networking Layer (`src/networking/`):**
- `circuit.rs` - UDP circuit management with handshake state machine
- `transport.rs` - UDP transport with SOCKS5 proxy support  
- `protocol/` - Auto-generated message structs and codecs from `message_template.msg`
- Performance-aware packet aggregation, compression, and bandwidth limiting
- State progression: NotStarted → UseCircuitCode → CompleteAgentMovement → RegionHandshake → RegionHandshakeReply → AgentThrottle → AgentUpdate → HandshakeComplete

**Rendering Pipeline (`src/rendering/`):**
- Built on `wgpu` with Hierarchical-Z Buffer (HZB) and Clustered Forward Shading  
- Dynamic quality adjustment based on performance profiles
- Three performance profiles: Low (optimized for low-end), Balanced (mid-range), High (maximum quality)
- WGSL-based shader programs with multiple quality variants

**Asset Management (`src/assets/`):**
- Virtual texturing system for efficient VRAM usage
- Dynamic mesh simplification and impostor generation
- Memory-aware caching with profile-based size limits  
- Progressive asset streaming coordinated with network layer

**Performance Configuration System (`src/config/`):**
- Hardware detection and automatic profile selection
- Runtime settings adjustment with adaptive scaling
- Granular controls for rendering, memory, and networking parameters

**Concurrency Model:**
- Data-Oriented Design (DOD) with job-based parallelism (`rayon`)
- Async I/O pool (`tokio`) for network and disk operations
- Channel-based message passing for loose coupling between subsystems

### Technology Stack

**Core Libraries:**
- `tokio` (1.46.0) - Async runtime
- `wgpu` (24.0.5) - Graphics API abstraction
- `eframe` (0.31) - Immediate-mode GUI (egui-based)
- `quinn` (0.11.8) - QUIC protocol support
- `glam` (0.30.4) - High-performance math

**Specialized:**
- `jpeg2k-sandboxed` (0.9.1) - SL texture decoding
- `collada` (0.16.0) - 3D model format
- `hound` (3.5.1) - Audio processing

### Protocol Implementation

The Second Life protocol implementation uses `message_template.msg` as the single source of truth. The current implementation includes:
- **Auto-generated message structs:** 483 messages parsed and generated at build time
- **Code generation system:** `build.rs` creates `messages.rs` and `codecs.rs` from template
- **Generated types:** `GeneratedMessage`, `GeneratedMessageCodec`, `GeneratedPacketHeader`
- **Legacy compatibility:** Co-exists with existing Message enum during transition
- **UDP-based communication:** Binary serialization, retransmission, circuit state management

### Reference Implementation

`hippolyzer/` contains a comprehensive Python-based Second Life protocol implementation that serves as:
- Protocol behavior reference
- Development and debugging tool
- Proxy server for testing

### Performance Targets & Scaling

**Performance Profiles:**
- **Low Profile:** Optimized for <4GB RAM, integrated graphics, aggressive bandwidth/memory saving
- **Balanced Profile:** Default for mid-range systems (4-16GB RAM, dedicated GPU)  
- **High Profile:** Maximum quality for high-end hardware (>16GB RAM, high-end GPU)
- **Custom Profile:** User-defined granular controls

**Target Metrics:**
- 60 FPS minimum rendering across all profiles
- Memory usage scaled to hardware capabilities (512MB-8GB+ asset caches)
- <10s startup time with adaptive first-run hardware detection
- <100ms network latency with profile-appropriate compression/aggregation
- Data-Oriented Design (DOD) principles for cache-friendly performance

## Development Notes

**Current Phase:** Phase 1B (Core Concurrency Model & Performance Configuration)
- **Completed:** Protocol parsing ✅, Code generation ✅  
- **Next:** Implement DOD concurrency model and performance configuration system

**Message Handling:** Use generated message types (`GeneratedMessage`, `GeneratedMessageCodec`) for all new development. Legacy `Message` enum exists for compatibility during transition.

**Performance Integration:** All new systems must integrate with the performance configuration system:
- Read settings from `PerformanceSettings` 
- Respect memory limits and quality profiles
- Provide hooks for runtime adjustment

**Rendering:** Target wgpu with HZB culling and clustered forward shading. Multiple shader variants per quality level.

**Asset Loading:** Design for virtual texturing, dynamic mesh simplification, and impostor generation. All systems must respect profile-based memory limits.

**Error Handling:** Uses `anyhow` for application errors and `thiserror` for library errors. Maintain this pattern for consistency.

**Testing Strategy:** Include unit tests, integration tests, performance benchmarks, and load tests for all major systems.

## Project Documentation References

- **`int.md`** - Unified integration plan and 4-phase roadmap (primary reference)
- **`main_plan.md`** - Original 5-phase networking plan (subset of int.md)
- **`perf.md`** - Performance architecture principles
- **GitHub Issues #1-#8** - Detailed implementation plans for each phase
- **`readme.md`** - Overall project concept and structure