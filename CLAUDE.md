# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

slv-rust is a Second Life viewer implementation in Rust, currently in v0.3.0-alpha development. The project aims to create a high-performance virtual world client with 60 FPS minimum performance and modern Rust architecture patterns.

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

The codebase follows a **5-phase implementation plan** detailed in `main_plan.md`:
1. Protocol Definition Parsing (`message_template.msg` as single source of truth)
2. Automated Code Generation (via build.rs - planned)
3. Connection & State Management
4. Application Integration (message-passing between layers)
5. Testing Strategy

### Key Architectural Components

**Networking Layer (`src/networking/`):**
- `circuit.rs` - UDP circuit management with handshake state machine
- `transport.rs` - UDP transport with SOCKS5 proxy support
- `protocol/` - Second Life protocol implementation (binary, little-endian)
- State progression: NotStarted → UseCircuitCode → CompleteAgentMovement → RegionHandshake → RegionHandshakeReply → AgentThrottle → AgentUpdate → HandshakeComplete

**Rendering Pipeline (`src/rendering/`):**
- Built on `wgpu` for cross-platform graphics (Vulkan/DirectX/Metal)
- Scene graph management with hierarchical culling
- WGSL-based shader programs in `shaders/`

**Asset Management (`src/assets/`):**
- Multi-format support: JPEG2000 (SL textures), Collada DAE, audio
- LRU caching with configurable size limits
- Progressive asset streaming

**Concurrency Model:**
- Async-first design using tokio runtime
- Channel-based message passing between subsystems
- Arc/Mutex patterns for shared state management

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

The Second Life protocol implementation uses `message_template.msg` as the authoritative source. Current focus is on UDP-based communication with:
- Binary message serialization/deserialization
- Automatic retransmission and acknowledgment
- Circuit state management for reliable communication

### Reference Implementation

`hippolyzer/` contains a comprehensive Python-based Second Life protocol implementation that serves as:
- Protocol behavior reference
- Development and debugging tool
- Proxy server for testing

### Performance Targets

- 60 FPS minimum rendering
- <2GB RAM usage
- <10s startup time
- <100ms network latency
- Data-Oriented Design (DOD) principles over OOP

## Development Notes

**Current Phase:** Phase 3 (Connection & State Management) transitioning to Phase 4 (Application Integration)

**Message Handling:** The networking layer implements a state machine for Second Life's handshake protocol. When working with protocol messages, refer to the state transitions in `src/networking/circuit.rs`.

**Rendering:** Uses modern graphics patterns with uniform buffers and descriptor sets. New shaders should be written in WGSL and placed in `src/rendering/shaders/`.

**Asset Loading:** All asset types go through the centralized asset manager. New asset types should implement the appropriate traits and integrate with the LRU cache system.

**Error Handling:** Uses `anyhow` for application errors and `thiserror` for library errors. Maintain this pattern for consistency.

## Project Documentation References

- Reference `readme.md` for overall project concept and structure
- Reference `perf.md` for overarching long-term performance targets and optimization architecture