# Development Journal

This document serves as our source of truth for tracking roadblocks, recurring bugs, and development bottlenecks encountered during slv-rust development. Every significant issue must be documented with context, attempted solutions, and final resolution to build institutional knowledge.

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

### Key Reference Files
- `homunculus/packages/homunculus-core/src/network/authenticator.ts` - Authentication patterns
- `message_template.msg` - Official SecondLife protocol messages
- `hippolyzer/` directory - Python reference implementation