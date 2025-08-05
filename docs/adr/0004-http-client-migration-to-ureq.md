# ADR-0004: HTTP Client Migration from Reqwest to Ureq

**Date**: 2025-08-03

**Status**: Accepted

## Context

The `slv-rust` project requires HTTP proxy support for integration with Hippolyzer, a debugging tool essential for SecondLife protocol development. Hippolyzer acts as a proxy server that captures and logs HTTP/HTTPS requests (login authentication, capabilities fetching) and UDP traffic (game protocol) for debugging and reverse engineering purposes.

During the authentication system implementation, we discovered that HTTP requests were not appearing in Hippolyzer proxy logs despite successful authentication. This created a critical blind spot in our debugging capabilities since SecondLife authentication uses XML-RPC over HTTPS.

The primary requirements are:
- **Reliable Proxy Support**: HTTP requests must be routed through proxy servers without silent fallbacks
- **Debugging Visibility**: All HTTP traffic must be visible in proxy logs for protocol analysis
- **Protocol Compliance**: Must maintain compatibility with SecondLife's XML-RPC authentication system
- **Integration with Hippolyzer**: Must work seamlessly with Hippolyzer's HTTP proxy (port 9062)

## Problem

Extensive testing revealed that `reqwest` implements a "resilience over control" philosophy with silent proxy fallback behavior that cannot be disabled:

1. **Silent Fallback**: When proxy configuration fails, times out, or encounters any issue, `reqwest` automatically falls back to direct connection without error reporting
2. **Configuration Ignored**: Multiple proxy configuration attempts failed:
   - `reqwest::Proxy::http()` - completely ignored
   - `reqwest::Proxy::all()` - inconsistent behavior
   - `.no_proxy()` and timeout configurations - no effect
   - `.no_proxy(reqwest::NoProxy::from_string(""))` - still bypassed

3. **Verification Test**: Created a local test proxy server that confirmed `reqwest` never attempts to connect to configured proxies

This behavior makes `reqwest` unsuitable for SecondLife viewer development where HTTP proxy logging is essential for protocol debugging.

## Decision

We have decided to migrate from `reqwest` to `ureq` for all HTTP client functionality.

### Rationale

#### `ureq` Advantages

- **Explicit Proxy Control**: `ureq` provides deterministic proxy behavior without silent fallbacks
- **Better Error Reporting**: Clear error messages when proxy connections fail rather than silent fallback
- **Synchronous API**: Simpler to debug and reason about, especially for XML-RPC requests
- **Proven Compatibility**: Known to work correctly with HTTP proxy servers like Hippolyzer
- **Smaller Footprint**: Less complex than `reqwest` with fewer dependencies

#### Migration Scope

The following components require refactoring:
- `src/networking/auth/xmlrpc.rs` - XML-RPC client for SecondLife authentication
- `src/networking/auth/login.rs` - Capabilities fetching HTTP client
- Error handling throughout the authentication chain
- Proxy configuration in authentication service

## Implementation Plan

1. **Phase 1**: Update dependencies and add `ureq` to `Cargo.toml`
2. **Phase 2**: Refactor XML-RPC client to use `ureq::Agent` with proxy configuration
3. **Phase 3**: Refactor capabilities client to use same `ureq` pattern
4. **Phase 4**: Update error handling to use `ureq::Error` types
5. **Phase 5**: Test proxy functionality with Hippolyzer integration
6. **Phase 6**: Remove `reqwest` dependency if no longer needed

## Consequences

### Positive

- **Reliable Proxy Support**: HTTP requests will definitively go through configured proxies
- **Better Debugging**: All HTTP traffic will be visible in Hippolyzer logs for protocol analysis
- **Explicit Failures**: Proxy connection failures will be reported rather than silently bypassed
- **Simplified API**: Synchronous API reduces complexity for authentication flows
- **Protocol Debugging**: Enables comprehensive SecondLife protocol reverse engineering

### Negative

- **Migration Effort**: Requires refactoring existing HTTP client code and error handling
- **API Differences**: Need to adapt from `reqwest`'s async API to `ureq`'s sync API  
- **Ecosystem**: `ureq` has a smaller ecosystem compared to `reqwest`
- **Learning Curve**: Team needs to learn `ureq` API patterns

### Migration Risks

- **Breaking Changes**: Authentication system temporarily unavailable during migration
- **Error Handling**: Need to carefully map `reqwest::Error` to `ureq::Error` patterns
- **Testing**: Requires comprehensive testing of proxy scenarios

## Alternatives Considered

- **`hyper` with manual proxy**: More complex but gives maximum control
- **Custom HTTP implementation over SOCKS5**: Significant development effort
- **Accept broken proxy logging**: Unacceptable for SecondLife protocol debugging
- **Different proxy tool**: Problem is with `reqwest`, not Hippolyzer

## References

- **Issue Documentation**: `DEVELOPMENT_JOURNAL.md` - "2025-08-03 - Reqwest HTTP Proxy Silent Fallback Issue"  
- **Test Methodology**: Created local proxy server to verify HTTP client behavior
- **Hippolyzer Integration**: HTTP proxy port 9062, SOCKS5 proxy port 9061
- **ureq Documentation**: https://docs.rs/ureq/latest/ureq/
- **Proxy Configuration**: `ureq::AgentBuilder::new().proxy(ureq::Proxy::new())`