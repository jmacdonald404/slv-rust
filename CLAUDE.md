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

## Development Notes

**Shortcuts and Tricks:**
- Just run instead of build/check, you get the same logs anyway and i wanna save tokens

**Current Phase:** Phase 1B (Core Concurrency Model & Performance Configuration)
- **Completed:** Protocol parsing ✅, Code generation ✅  
- **Next:** Implement DOD concurrency model and performance configuration system

[... rest of the existing content remains unchanged ...]