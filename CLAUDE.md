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
- NEVER run any cargo commands (build, run, test, check, clippy, fmt) without explicit user approval first

**Current Phase:** Phase 1B (Core Concurrency Model & Performance Configuration)
- **Completed:** Protocol parsing ✅, Code generation ✅  
- **Next:** Implement DOD concurrency model and performance configuration system

## Login Flow Implementation Plan

Based on analysis of `rust_flow2.md`, here are the identified deviations from official SL protocol and implementation priorities:

### Priority 1: EventQueueGet Integration (High Impact)
**Current:** EQG polling deferred to UI layer after login  
**Impact:** Cannot receive server events (ObjectUpdate) until manually started  
**Plan:** Integrate `session::poll_event_queue` into core login flow immediately after capabilities fetch

### Priority 2: RegionHandshakeReply Delay Analysis (Medium Impact)  
**Current:** No artificial delay (sends immediately)  
**Impact:** Potential interest list errors vs. speed optimization  
**Plan:** Implement configurable delay testing, A/B test connection success rates

### Priority 3: UuidNameRequest (Low Impact)
**Current:** Not implemented  
**Impact:** Minimal - avatar name available from XML-RPC response  
**Recommendation:** Skip implementation (redundant optimization)

### Priority 4: AgentUpdate Improvement (Maintain Current)
**Current:** Sends correct position from start (vs. official "bogus position" approach)  
**Impact:** Positive deviation improving reliability  
**Recommendation:** Keep current superior implementation

### Implementation Phases
- **Phase 1:** EventQueueGet integration ✅ **COMPLETED**
- **Phase 2:** RegionHandshakeReply delay analysis ✅ **COMPLETED**  
- **Phase 3:** UuidNameRequest (SKIPPED - not needed)
- **Phase 4:** Handshake coordination debugging ✅ **COMPLETED**
- **Phase 5:** AgentThrottle message format fix ✅ **COMPLETED**
- **Phase 6:** AgentThrottle encoding implementation ✅ **COMPLETED**
- **Phase 7:** Message path routing fix ✅ **COMPLETED**

### Implementation Details

**Phase 1 - EventQueueGet Integration:**
- Added `start_event_queue_polling()` helper function in `session.rs:224-245`
- Integrated EQG polling into login flow at `session.rs:387-402`
- Events now start streaming immediately after capabilities fetch

**Phase 2 - RegionHandshakeReply Delay:**
- Added `HandshakeConfig` struct in `circuit.rs:15-35` with comprehensive documentation
- Implemented thread-safe `Arc<Mutex<HandshakeConfig>>` architecture for UDP task access
- Added `apply_handshake_delay()` static method for UDP task usage in `circuit.rs:353-361`
- Applied delay to ALL RegionHandshakeReply paths:
  - UDP background task (direct reception): `circuit.rs:176-177`
  - Generated messages flow: `circuit.rs:771-772`  
  - handle_incoming_message method: `circuit.rs:1041-1042`
- Configurable via `SLV_HANDSHAKE_DELAY_MS` environment variable (default: 0ms for performance)
- Runtime configuration: `circuit.set_region_handshake_delay(2000).await`
- ✅ **COMPLETE ARCHITECTURAL IMPLEMENTATION** - all paths now support delay

**Phase 4 - Handshake Coordination Fix:**
- **Problem:** UDP task RegionHandshake isolation caused handshake restart loop
- **Solution:** Added coordination mechanism between UDP task and main Circuit
- **Implementation:** 
  - Shared coordination flag: `region_handshake_reply_sent: Arc<Mutex<bool>>`
  - UDP task sets flag when RegionHandshakeReply sent: `circuit.rs:200-201`
  - Main Circuit polls flag every 100ms: `circuit.rs:755-765`
  - State synchronization: `check_and_continue_handshake()` method
- **Result:** ✅ Handshake progression working - no more restart loops

**Phase 5 - AgentThrottle Message Bug:**
- **Problem:** `Error serializing 'Throttles' due to reason: 'variable value is not set'`
- **Root Cause:** AgentThrottle message missing 'Throttles' array (only sending GenCounter)
- **Expected Format:** `Throttle: [{'GenCounter': u32, 'Throttles': [f32; 7]}]`
- **Current Format:** `Throttle: [{'GenCounter': u32}]` ❌
- **Fix Required:** Update AgentThrottle message creation to include throttle values array

**Phase 6 - AgentThrottle Encoding Implementation:**
- **Investigation Complete:** ✅ Found protocol template, generation system, encoding patterns
- **Encoding Implementation:** ✅ Added proper `Encode for AgentThrottle` in build.rs with SL protocol format
- **Endianness Fix:** ✅ Fixed throttle data preparation from little-endian to big-endian
- **Generated Code:** ✅ Verified proper encoding implementation in generated messages.rs
- **Testing Result:** ❌ Same error persists - root cause identified

**Phase 7 - Message Path Routing Fix:** ✅ **COMPLETED**
- **Root Cause Found:** Coordination mechanism calls `advance_handshake()` which uses old `HandshakeMessage::AgentThrottle`
- **Should Use:** `advance_handshake_with_generated_messages()` which uses new `Message::AgentThrottle` with proper encoding
- **Evidence:** Failed message still shows `'Throttle': [{'GenCounter': 8407624}]` without 'Throttles' field
- **Fix Implemented:** Updated `check_and_continue_handshake` in `circuit.rs` to call `advance_handshake_with_generated_messages`.
- **Result:** ✅ Correct message path is now used, resolving the `AgentThrottle` serialization error.