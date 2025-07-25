# Phase 3: Connection & State Management

## Objective
Refactor the existing networking modules (`circuit.rs`, `transport.rs`, `utils/lludp.rs`) to fully integrate and utilize the auto-generated message structs and codecs from Phase 2. This phase will solidify the `Circuit` as the definitive manager of a simulator connection, handling state, reliability, and the flow of messages.

## Files to Modify
*   `src/networking/circuit.rs` (Major changes)
*   `src/networking/transport.rs` (Simplification)
*   `src/networking/utils/lludp.rs` (Deprecation/Removal)

## Key Tasks

### 1. Remove Manual Packet Builders
The various `build_*_packet` functions in `src/utils/lludp.rs` and the corresponding sender methods in `src/networking/transport.rs` (e.g., `send_usecircuitcode_packet_lludp`) are now obsolete.
*   **Action:** Delete these functions. The `MessageCodec::encode` function generated in Phase 2 is the new single source of truth for serialization.

### 2. Centralize and Generalize Message Sending
All outgoing messages should be sent through a single, unified function in the `Circuit`.
*   **Action:** In `src/networking/circuit.rs`, refactor `send_message` into the primary method for sending data. Its signature should be approximately:
    ```rust
    pub async fn send_message(&mut self, message: Message, reliable: bool) -> io::Result<()>;
    ```
*   **Implementation:**
    *   This function will increment the circuit's sequence number.
    *   It will call the generated `MessageCodec::encode` to serialize the `Message` enum into a byte buffer.
    *   It will prepend the necessary LLUDP packet header (sequence number, flags for reliability, etc.).
    *   If `reliable` is true, it will store a copy of the message in the `unacked_messages` map for potential retransmission.
    *   Finally, it will send the byte buffer using the `UdpTransport`.

### 3. Refactor the Handshake Logic
The `advance_handshake` state machine in `circuit.rs` needs to be updated to use the new, generalized `send_message` function.
*   **Action:**
    *   Replace calls like `transport.send_complete_agent_movement_packet(...)` with:
    ```rust
    let message = Message::CompleteAgentMovement(CompleteAgentMovement {
        // ... populate fields ...
    });
    self.send_message(message, true).await?;
    ```
    *   This makes the handshake logic cleaner and ensures it uses the same serialization path as all other messages.

### 4. Refine the Main Receive Loop
The background task spawned in `Circuit::new_with_transport` is responsible for receiving raw UDP data. It needs to be updated to use the new codec.
*   **Action:**
    *   The loop will continue to receive raw byte packets from the `UdpTransport`.
    *   It will pass the entire packet payload to the generated `MessageCodec::decode`.
    *   `decode` will return a `Result<(PacketHeader, Message)>`.
    *   The logic for handling ACKs, packet ordering, and reliability remains, but it now operates on the fully structured `Message` enum instead of raw bytes.
    *   Decoded messages are then passed to the `handle_incoming_message` function or a new dispatcher.

### 5. Update State Management
The `Circuit` struct will continue to be the owner of all connection-related state.
*   **`next_sequence_number`**: The counter for outgoing packets.
*   **`unacked_messages`**: The buffer for reliable messages awaiting an ACK. The retransmission logic remains the same.
*   **`next_expected_sequence_number`**: The counter for incoming ordered packets.
*   **`out_of_order_buffer`**: The buffer for out-of-order packets.
*   **`handshake_state`**: The state machine for the initial connection sequence.

By the end of this phase, the `Circuit` will be a self-contained, robust manager for a single simulator connection, capable of sending and receiving any message defined in the protocol, while correctly handling the entire handshake and reliability layer.
