# Lock Contention Analysis and Fix

## Problem Identified

The networking branch shows "Timeout acquiring UDP socket lock - possible contention" errors because:

1. **Lock Timeout Too Short**: 100ms timeout on UDP socket locks in `src/networking/proxy/socks5.rs:468`
2. **Multiple Concurrent Senders**: Multiple threads trying to send simultaneously
3. **Lock Contention**: The `Arc<Mutex<Option<UdpSocket>>>` is being heavily contended

## Root Cause in Networking Branch

```rust
// From networking branch - socks5.rs:468
let socket_guard = tokio::time::timeout(
    std::time::Duration::from_millis(100),  // TOO SHORT!
    self.udp_socket.lock()
).await.map_err(|_| NetworkError::Transport {
    reason: "Timeout acquiring UDP socket lock - possible contention".to_string()
})?;
```

## Main Branch Works Because

Main branch has simpler architecture:
- Direct SOCKS5 socket without complex locking
- No timeout on socket operations 
- Simpler `UdpSocketExt` trait implementation

## Iterative Fix Strategy

### Step 1: Increase Timeout
Change 100ms to 5000ms (5 seconds) in networking branch

### Step 2: Remove Unnecessary Locking
Consider if `Arc<Mutex<Option<UdpSocket>>>` is really needed

### Step 3: Connection Pooling
Use connection pool instead of single shared socket

### Step 4: Async Channel Pattern
Replace mutex with async channel for sending

## Test Command
```bash
# Switch to networking branch and test
git checkout networking
cargo build
# Look for "Timeout acquiring UDP socket lock" errors in logs
```