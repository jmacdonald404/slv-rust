# Issues Log

## Issue 1: Client-Server Communication Hang

**Description:**
After implementing basic UDP transport, circuit management, and message serialization/deserialization, the client-server communication test in `main.rs` hangs after the client sends a `KeepAlive` message. The server's `recv_message` does not appear to be triggered.

**Steps to Reproduce:**
1. Ensure `main.rs` contains the client-server test code.
2. Run `cargo run`.

**Expected Behavior:**
Both client and server should successfully send and receive messages, and the program should exit gracefully after the echo is complete.

**Actual Behavior (Initial):**
"Server listening" and "Client sent" messages are printed, but the program then hangs, indicating the server is not receiving the message or the client is not receiving the echo.

**Root Cause (Identified):**
The server's `UdpTransport` was binding to an ephemeral port (`0.0.0.0:xxxxx`) instead of the intended `127.0.0.1:8080`. The client was correctly sending to `127.0.0.1:8080`, leading to a mismatch.

**Resolution:**
Modified `Circuit::new` to accept a `bind_addr` parameter, allowing explicit control over the local address the UDP socket binds to. Updated `main.rs` to ensure the server binds to `127.0.0.1:8080` and the client binds to `0.0.0.0:0` (ephemeral). The `cargo run` output confirms successful message exchange:
```
UdpTransport bound to: 127.0.0.1:8080
Server listening on 127.0.0.1:8080
UdpTransport bound to: 0.0.0.0:53432
Client sent KeepAlive message to 127.0.0.1:8080
Server received: PacketHeader { sequence_id: 1, flags: 0 }, KeepAlive from 127.0.0.1:53432
Server echoed message back to 127.0.0.1:53432.
Client received echo: PacketHeader { sequence_id: 1, flags: 0 }, KeepAlive
```

## Issue 2: Implement Packet Reliability

**Description:**
The current networking layer lacks packet reliability, meaning packets can be lost, duplicated, or arrive out of order. This issue tracks the implementation of automatic retransmission and ordering.

**Sub-tasks:**
- Introduce sequence numbers for all outgoing packets.
- Implement Acknowledgements (ACKs) for received packets.
- Implement Retransmission logic for unacknowledged packets.
- Handle out-of-order packet delivery.

**Priority:** High