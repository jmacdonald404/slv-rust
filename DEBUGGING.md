# Common Debugging Issues - slv-rust SecondLife Viewer

This document outlines the most common debugging issues encountered during the development of slv-rust, a SecondLife viewer implementation in Rust.

## Table of Contents

1. [Protocol & Networking Issues](#protocol--networking-issues)
2. [Rendering & Graphics Issues](#rendering--graphics-issues)
3. [Asset Management Issues](#asset-management-issues)
4. [Build & Compilation Issues](#build--compilation-issues)
5. [Logging & Debugging Tools](#logging--debugging-tools)
6. [Performance Issues](#performance-issues)

## Protocol & Networking Issues

### 1. Circuit Connection Failures

**Problem**: Connection to SecondLife simulators fails during handshake.

**Common Causes**:
- Invalid circuit code received from login server
- SOCKS5 proxy configuration issues
- UDP packet loss during handshake

**Debugging Steps**:
```bash
# Enable network logging
RUST_LOG=debug,slv_rust::networking=trace cargo run

# Check for specific handshake messages
grep -i "handshake\|circuit" log.txt
```

**Key Files**: 
- `src/networking/circuit.rs:47` - Circuit acknowledgment manager
- `src/networking/handlers/region_handlers.rs` - RegionHandshakeReply handling
- `src/networking/auth/session.rs` - Session management

### 2. Packet Serialization/Deserialization Errors

**Problem**: Messages fail to serialize or deserialize properly according to SL protocol.

**Common Issues**:
- Incorrect message template parsing
- Endianness issues with binary data
- Variable block encoding problems

**Debug Files**:
- Generated protocol definitions: `src/networking/packets/generated.rs`
- Message templates in `external/master-message-template/`

**Fix History**: 
- Commit `374357f` - UseCircuitCode packet formatting fixes
- Commit `0d2dfae` - SL protocol codec and ACK packet handling

### 3. Message Template Issues

**Problem**: Protocol messages don't match expected format from SecondLife servers.

**Root Cause**: Outdated or incorrectly parsed message templates.

**Solution**:
```bash
# Regenerate protocol definitions
cargo clean
cargo build  # Triggers build.rs template parsing
```

## Rendering & Graphics Issues

### 1. wgpu Initialization Failures

**Problem**: Graphics backend fails to initialize, especially on older hardware.

**Common Errors**:
- Adapter not found
- Feature requirements not met
- Surface creation failures

**Debugging**:
```bash
# Enable wgpu logging
WGPU_LOG=1 RUST_LOG=debug cargo run

# Check adapter info
grep -i "adapter\|wgpu" log.txt
```

**Key Files**:
- `src/utils/logging.rs:145` - Adapter info logging
- `src/rendering/engine.rs` - Main rendering engine

### 2. Shader Compilation Issues

**Problem**: Shaders fail to compile on different graphics backends.

**Common Causes**:
- Backend-specific WGSL differences
- Feature compatibility issues
- Missing texture bindings

**Key Files**:
- `src/rendering/shaders/` - Shader implementations
- `src/rendering/shaders/quality_variants.rs` - Platform-specific variants

### 3. Performance Renderer Integration

**Problem**: Performance renderer fails with modern wgpu versions.

**Issues**:
- Depth buffer configuration
- Texture bind group caching
- HZB (Hierarchical Z-Buffer) implementation

**Key File**: `src/rendering/performance_renderer.rs:118` - Texture bind group creation

## Asset Management Issues

### 1. JPEG2000 Texture Decoding

**Problem**: Texture assets fail to decode from SL's JPEG2000 format.

**Common Issues**:
- `jpeg2k-sandboxed` crate compatibility
- Memory allocation failures for large textures
- Progressive loading interruptions

**Key Files**:
- `src/assets/texture.rs` - Texture processing pipeline
- `src/assets/manager.rs` - Asset loading coordination

### 2. Mesh Loading Failures

**Problem**: 3D mesh assets fail to load or render incorrectly.

**Causes**:
- Collada DAE parsing issues
- SL native mesh format incompatibilities
- Scene graph integration problems

**Key Files**:
- `src/assets/mesh.rs` - Mesh loading implementation
- `src/rendering/scene/graph.rs:24` - Scene graph object management

## Build & Compilation Issues

### 1. Message Template Generation

**Problem**: Build fails during protocol generation phase.

**Root Cause**: Template parsing errors in `build.rs`.

**Fix**: Check `external/master-message-template/message_template.msg` for format issues.

**Key Files**:
- `build.rs` - Build-time code generation
- `src/utils/build_utils/template_parser.rs` - Template parsing logic

### 2. Dependency Version Conflicts

**Problem**: Crate version incompatibilities, especially with async/graphics crates.

**Common Conflicts**:
- `wgpu` version mismatches
- `tokio` feature flag conflicts
- `bincode` serialization version issues

**Recent Fixes**:
- Commit `17fc759` - Compilation error fixes and build stabilization
- Commit `90e0387` - Render pipeline initialization stability

## Logging & Debugging Tools

### 1. Comprehensive Logging Setup

The project uses `tracing` for structured logging with multiple output targets:

```rust
// Enable different log levels
RUST_LOG=debug cargo run          # General debugging
RUST_LOG=trace cargo run          # Verbose networking
WGPU_LOG=1 cargo run              # Graphics debugging
RUST_BACKTRACE=1 cargo run        # Panic backtraces
```

**Log Output**:
- Console output with colors and timestamps
- `log.txt` file (cleaned on each startup)
- Panic handler with location and backtrace

**Key Features** (`src/utils/logging.rs`):
- Per-module log level control
- WGPU-specific debugging options
- System information logging
- Thread-aware logging with IDs and names

### 2. Network Debugging

**Circuit Debug Info**:
```bash
# Monitor circuit establishment
grep -i "circuit\|handshake" log.txt

# Check packet acknowledgments
grep -i "ack\|reliable" log.txt

# Monitor login process
grep -i "login\|auth" log.txt
```

### 3. Graphics Debugging

**wgpu Debug Output**:
```bash
# Adapter selection
grep -i "adapter.*information" log.txt

# Device capabilities
grep -i "device.*information" log.txt

# Render pipeline issues
grep -i "pipeline\|shader" log.txt
```

## Performance Issues

### 1. Frame Rate Drops

**Common Causes**:
- Inefficient scene graph traversal
- Texture thrashing in asset cache
- GPU memory pressure

**Monitoring**:
- Built-in performance renderer metrics
- Frame time logging in render loop

### 2. Memory Usage

**Problem Areas**:
- Asset cache growing unbounded
- GPU buffer leaks
- Network buffer accumulation

**Debug Tools**:
- System information logging on startup
- Memory usage tracking in asset manager

## Quick Debugging Checklist

When encountering issues, follow this systematic approach:

1. **Enable Debug Logging**:
   ```bash
   RUST_LOG=debug WGPU_LOG=1 RUST_BACKTRACE=1 cargo run
   ```

2. **Check Recent Commits**: Review git history for similar issues:
   ```bash
   git log --oneline --grep="fix\|debug\|error" -10
   ```

3. **Examine Log File**: Look for specific error patterns:
   ```bash
   tail -f log.txt | grep -i "error\|panic\|failed"
   ```

4. **Test Components Individually**:
   - Network: Run networking examples
   - Graphics: Test with minimal scene
   - Assets: Verify cache directory permissions

5. **Check Environment**: Ensure proper system setup:
   - Graphics drivers updated
   - Network connectivity
   - File system permissions

## Contributing Debug Information

When reporting issues:

1. Include full `log.txt` output
2. Specify hardware/OS configuration
3. List exact steps to reproduce
4. Note any recent changes or commits
5. Include relevant environment variables

## TODO Items Requiring Debug Attention

The codebase contains numerous TODO items that may cause issues:

- **Physics Integration**: `src/world/physics.rs` - Physics engine stubs
- **Asset System**: `src/assets/manager.rs` - Network request channels
- **Rendering**: Scene graph object management and transforms
- **UI Components**: Preference saving and state management

Monitor these areas for potential debugging needs as development progresses.