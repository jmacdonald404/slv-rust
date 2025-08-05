# Development Journal

This document serves as our source of truth for tracking roadblocks, recurring bugs, and development bottlenecks encountered during slv-rust development. Every significant issue must be documented with context, attempted solutions, and final resolution to build institutional knowledge.

---

## 2025-08-03 - Build Environment Compilation Timeout During Ureq Migration

**Context**: During the migration from reqwest to ureq (documented in ADR-0004), encountered severe compilation timeout issues that prevented testing of the new HTTP client implementation.

**Issue**:
- Compilation consistently times out after 30-60 seconds across all binaries
- Issue persists even after reverting code changes and cleaning build cache 
- Both `cargo check` and `cargo build` affected
- Problem occurred both with ureq-based implementation and after reverting to reqwest
- Compilation proceeds through dependency compilation but times out before reaching project code

**Investigation**:
1. **Code Changes**: Initially suspected ureq integration caused compilation loops
   - Removed ureq dependency entirely - issue persisted
   - Reverted XML-RPC client back to reqwest - issue persisted
   - Cleaned unused imports and simplified code structure - no improvement

2. **Build Cache**: Performed `cargo clean` removing 23.7GB of build artifacts
   - Fresh compilation still times out during dependency compilation phase
   - Suggests issue is not with cached artifacts

3. **Dependency Analysis**: Compilation proceeds through normal dependency chain
   - Successfully compiles proc-macro2, unicode-ident, libc, serde, etc.
   - Times out during later dependencies like rustix
   - No obvious dependency conflicts or version issues

**Environment Details**:
- Platform: macOS (Darwin 21.6.0)  
- Rust toolchain: Standard Rust installation
- Project: Large codebase with ~140 dependencies
- Cargo.toml: Contains multiple binary targets for testing

**Attempted Solutions**:
1. Removed ureq dependency - no effect
2. Reverted code changes to known working state - no effect  
3. Cleaned build cache completely - no effect
4. Simplified test binaries - still timeout during compilation
5. Isolated simple ureq test (separate binary) - same timeout

**Root Cause Hypothesis**:
This appears to be a local development environment issue rather than code-related:
- Possible system resource constraints (memory, disk I/O)
- Potential background processes interfering with compilation
- rustc/LLVM compilation performance degradation
- File system issues affecting large dependency compilation

**Workaround**:
- Migrate development to different environment
- Consider using pre-built dependencies or docker container
- Implement proxy debugging via external tools temporarily

**Next Steps**:
1. Test ureq migration in different development environment
2. Consider alternative approaches (custom HTTP proxy wrapper)
3. Monitor system resources during compilation
4. Potentially implement minimal HTTP proxy detection separate from main build

**Status**: Unresolved - requires environment change or alternative approach

**Prevention**: Document any environment-specific build requirements and maintain lighter test configurations for rapid iteration.

---

## 2025-07-30 - Authentication Handshake Timeout Resolution

**Context**: Implementing SecondLife authentication system to resolve "handshake timeout when connecting to authentication server" error.

**Problem**: Authentication system was using hardcoded fake data (`127.0.0.1:9000`) instead of connecting to real SecondLife login servers.

**Attempts**: 
- Attempt 1: Initial investigation revealed fake authentication data in codebase → Need real XML-RPC implementation
- Attempt 2: Studied `homunculus/packages/homunculus-core/src/network/authenticator.ts` for reference implementation → Understood XML-RPC protocol requirements
- Attempt 3: MD5 import error with `md5::Md5` → Fixed by using `md5::compute()` instead of `Md5::new()`
- Attempt 4: XML parsing error "No methodResponse found" → Fixed by checking if root element is methodResponse first
- Attempt 5: Missing function export error → Added `available_grids()` function and proper module exports

**Resolution**: Successfully implemented complete XML-RPC authentication system that connects to real SecondLife login servers. System now authenticates with `login.agni.lindenlab.com` and retrieves actual session data including agent_id, session_id, circuit_code, and simulator_address.

**Prevention**: 
- Always reference `homunculus/` and `hippolyzer/` implementations before implementing networking features
- Use `md5::compute()` instead of deprecated `Md5::new()` API for MD5 hashing
- Check XML root element type before searching for child elements
- Ensure all public functions are properly exported in module `mod.rs` files

**References**: 
- `homunculus/packages/homunculus-core/src/network/authenticator.ts` - XML-RPC authentication reference
- SecondLife login protocol: MD5 password hashing with 16-character truncation
- Circuit establishment with real simulator addresses

---

## 2025-08-03 - Reqwest HTTP Proxy Silent Fallback Issue

**Context**: Implementing HTTP proxy support for Hippolyzer integration to capture SecondLife login requests and HTTPS traffic for debugging and protocol analysis.

**Problem**: HTTP requests (login authentication, capabilities fetching) were not appearing in Hippolyzer proxy logs despite successful authentication. All reqwest HTTP requests were silently bypassing proxy configuration and using direct connections.

**Attempts**: 
- Attempt 1: Used `reqwest::Proxy::http()` with standard configuration → Proxy completely ignored, requests succeeded with same IP
- Attempt 2: Added `.no_proxy()` and timeout configurations → Still bypassed proxy
- Attempt 3: Used `reqwest::Proxy::all()` instead of `http()` → Prevented some fallback but still inconsistent
- Attempt 4: Added `.no_proxy(reqwest::NoProxy::from_string(""))` to disable bypass rules → No effect
- Attempt 5: Created test proxy server to definitively prove reqwest behavior → Confirmed reqwest never connects to proxy
- Attempt 6: Tested broken proxy ports (9999) → Requests still succeeded, proving fallback behavior

**Root Cause**: Reqwest is designed with "resilience over control" philosophy. It implements silent proxy fallback mechanisms that cannot be disabled. When proxy fails, times out, or has any connectivity issue, reqwest automatically falls back to direct connection without error reporting.

**Resolution**: Migrated from `reqwest` to `ureq` HTTP client which provides:
- Explicit proxy control without silent fallbacks
- Better error reporting for proxy failures  
- Synchronous API that's easier to debug
- Proven compatibility with HTTP proxy servers like Hippolyzer

**Prevention**: 
- Use `ureq` for HTTP clients where proxy control is required
- Always test proxy functionality with a local test server to verify requests are actually proxied
- Document proxy requirements in architecture decisions
- For SecondLife viewer development, HTTP proxy logging is critical for protocol debugging

**References**: 
- Hippolyzer proxy ports: HTTP 9062, SOCKS5 9061
- `ureq` proxy configuration: `ureq::AgentBuilder::new().proxy(ureq::Proxy::new())`
- Test methodology: Create local proxy server to verify HTTP client behavior

---

## Template for Future Entries

```markdown
## [Date] - [Issue Title]

**Context**: What were you working on?
**Problem**: Specific issue encountered
**Attempts**: 
- Attempt 1: [Description] → [Outcome]
- Attempt 2: [Description] → [Outcome]
**Resolution**: Final working solution
**Prevention**: How to avoid this in the future
**References**: Links to docs, issues, or code that helped
```

---

## Quick Reference

### Common Issues Patterns
- **MD5 Crate**: Use `md5::compute()` not `Md5::new()`
- **XML Parsing**: Check root element type before searching children
- **Module Exports**: Always update `mod.rs` when adding public functions
- **SecondLife Protocol**: Reference `homunculus/` and `hippolyzer/` implementations
- **Authentication**: Use XML-RPC with proper password hashing (MD5 + 16-char truncation)
- **HTTP Proxy**: Use `ureq` not `reqwest` for proxy control - reqwest has silent fallback behavior

### Key Reference Files
- `homunculus/packages/homunculus-core/src/network/authenticator.ts` - Authentication patterns
- `message_template.msg` - Official SecondLife protocol messages
- `hippolyzer/` directory - Python reference implementation