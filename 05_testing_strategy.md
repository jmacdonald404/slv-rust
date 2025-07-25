# Phase 5: Testing Strategy

## Objective
To define and implement a multi-layered testing strategy that ensures the correctness, robustness, and reliability of the entire networking stack.

## Layer 1: Unit Tests
These tests focus on individual components in isolation.

*   **Protocol Parser (`template_parser.rs`)**
    *   **Goal:** Verify that the parser correctly interprets the `message_template.msg` format.
    *   **Tests:**
        *   Test parsing of a simple, well-formed message definition.
        *   Test parsing of messages with all `Frequency` types (Low, Medium, High, Fixed).
        *   Test parsing of blocks with all `Cardinality` types (Single, Multiple, Variable).
        *   Test that comments and empty lines are ignored correctly.
        *   Test that hex message IDs are parsed correctly.
        *   Test that malformed input produces a descriptive `Err`.

*   **Generated Codecs (`codecs.rs`)**
    *   **Goal:** Verify that the auto-generated serialization and deserialization logic is correct (round-trip testing).
    *   **Tests:**
        *   For a representative sample of messages (e.g., one simple, one complex, one with variable fields):
            1.  Manually construct an instance of the message's struct.
            2.  Use the generated `MessageCodec::encode` to serialize it to bytes.
            3.  Use a known, valid byte representation of that packet from a real-world example (e.g., from a Wireshark capture) and assert that the encoded bytes match.
            4.  Use the generated `MessageCodec::decode` to parse the known-good byte representation back into a struct.
            5.  Assert that the deserialized struct is equal to the original struct from step 1.

## Layer 2: Integration Tests
These tests verify that different parts of the networking stack work together correctly. They should be located in the `tests/` directory at the project root.

*   **Full Handshake Test**
    *   **Goal:** Verify that the client can successfully perform the entire UDP handshake sequence.
    *   **Setup:** This test may require a mock simulator or running against the actual Second Life grid.
    *   **Steps:**
        1.  Initiate a `Circuit`.
        2.  Call `advance_handshake` to send `UseCircuitCode`.
        3.  (Mock) Receive a valid reply from the simulator.
        4.  Call `advance_handshake` to send `CompleteAgentMovement`.
        5.  (Mock) Receive `RegionHandshake`.
        6.  Verify that the client automatically sends `RegionHandshakeReply`.
        7.  Verify that subsequent calls to `advance_handshake` send `AgentThrottle` and the first `AgentUpdate`.
        8.  Assert that the `handshake_state` is `HandshakeComplete` at the end.

*   **Reliability and Ordering Test**
    *   **Goal:** Verify that the `Circuit` correctly handles packet loss, ACKs, and out-of-order packets.
    *   **Setup:** Requires a mock simulator that can be controlled to drop or reorder packets.
    *   **Tests:**
        *   **Packet Loss:** Send a reliable message from the mock sim. Don't send an ACK from the client. Verify the mock sim retransmits the message.
        *   **ACKs:** Send a reliable message from the client. Verify the mock sim receives it. Send an ACK from the mock sim. Verify the message is removed from the client's `unacked_messages` buffer.
        *   **Ordering:** Send packets with sequence numbers 1, 3, and then 2 from the mock sim. Verify that the application layer (via the channels from Phase 4) receives the corresponding messages in the correct order (1, 2, 3).

## Layer 3: End-to-End (E2E) Tests
These are the highest-level tests that simulate a real user session.

*   **Login and In-World Test**
    *   **Goal:** Verify a complete user session from login to receiving basic world data.
    *   **Setup:** This test will run against the live Second Life grid and requires valid credentials (to be loaded from environment variables, not hardcoded).
    *   **Steps:**
        1.  Call `login_to_secondlife`. Assert that it returns `Ok` with valid session info.
        2.  Establish a `Circuit` and complete the handshake.
        3.  Listen on the application-level event channels (from Phase 4).
        4.  Assert that basic messages, such as `ObjectUpdate`, `ChatFromSimulator` (from the MOTD), and `AgentDataUpdate`, are received and successfully decoded within a reasonable time frame (e.g., 30 seconds).
        5.  Send a `ChatFromViewer` message and (if possible) verify it was sent.

This tiered approach ensures that bugs can be caught at the lowest possible level, making them easier to diagnose and fix, while also providing confidence that the entire system works as a whole.
