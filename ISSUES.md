# Issues Log

## Issue 1: Client-Server Communication Hang

**Description:**
After implementing basic UDP transport, circuit management, and message serialization/deserialization, the client-server communication test in `main.rs` hangs after the client sends a `KeepAlive` message. The server's `recv_message` does not appear to be triggered.

**Steps to Reproduce:**
1. Ensure `main.rs` contains the client-server test code.
2. Run `cargo run`.

**Expected Behavior:**
Both client and server should successfully send and receive messages, and the program should exit gracefully after the echo is complete.

**Actual Behavior:**
"Server listening" and "Client sent" messages are printed, but the program then hangs, indicating the server is not receiving the message or the client is not receiving the echo.

**Root Cause (Identified):**
The server's `UdpTransport` was binding to an ephemeral port (`0.0.0.0:xxxxx`) instead of the intended `127.0.0.1:8080`. The client was correctly sending to `127.0.0.1:8080`, leading to a mismatch.

**Resolution Attempt:**
Modified `Circuit::new` to accept a `bind_addr` parameter, allowing explicit control over the local address the UDP socket binds to. Updated `main.rs` to ensure the server binds to `127.0.0.1:8080` and the client binds to `0.0.0.0:0` (ephemeral).