# `slv-rust` Login Flow: Deviations from Official Spec

This document details the areas where the current `slv-rust` implementation deviates from or has not yet fully implemented the login sequence described in `official_flow.md`.

While `slv-rust` has a more robust and modern login flow in many respects, several steps from the traditional sequence are either missing, deferred, or handled differently.

## 1. Missing `UuidNameRequest` Packet

-   **Official Flow:** The viewer sends a `UuidNameRequest` packet early in the UDP handshake, typically after `CompleteAgentMovement`. This is done to request the avatar's own name, though the flow document notes it is "probably unnecessary, but traditional."
-   **`slv-rust` Implementation:** The `UuidNameRequest` packet is **not sent**. A search of the codebase reveals it is defined in the protocol template (`message_template.msg`) but is not implemented or called in the handshake logic in `src/networking/circuit.rs` or `src/networking/session.rs`.

    **Impact:** This is likely a minor deviation. The avatar's name is available from the initial XML-RPC login response, so this packet is redundant. Omitting it is a reasonable optimization.

## 2. Deferred `EventQueueGet` (EQG) Polling

-   **Official Flow:** The `EventQueueGet` capability is used to establish a long-polling HTTPS connection to the simulator *during* the login sequence, before the final `AgentUpdate`. This is the primary mechanism for receiving events from the simulator.
-   **`slv-rust` Implementation:** The logic for polling the event queue exists in `session::poll_event_queue`. However, in the main `login_to_secondlife` function, the comment explicitly states: `(Event queue polling should be started from the UI code after login succeeds)`. The polling is not initiated as part of the core login function.

    **Impact:** This is a significant architectural difference. By deferring the EQG polling, the initial login is simplified, but no events (like `ObjectUpdate`) can be received until the UI layer explicitly starts the polling task. This is a valid design choice, but it's a clear departure from the integrated flow described in the spec.

## 3. No Deliberate Delay Before `RegionHandshakeReply`

-   **Official Flow:** A note in the diagram suggests adding a 2-second delay before sending `RegionHandshakeReply` to "reduce interest list errors." This is a workaround for a known server-side or protocol-level race condition (see `BUG-233107`).
-   **`slv-rust` Implementation:** There is no such artificial delay in `src/networking/circuit.rs`. The `RegionHandshakeReply` is sent as soon as the `RegionHandshake` is received and processed.

    **Impact:** The `slv-rust` implementation prioritizes speed and does not include this legacy workaround. This could potentially lead to the interest list errors mentioned in the spec, but it's also possible that other architectural improvements (like fetching capabilities first) render this delay unnecessary. This is an area that would require careful testing.

## 4. Elimination of the "Bogus" `AgentUpdate`

-   **Official Flow:** The viewer sends an initial `AgentUpdate` with a "bogus" avatar position because it doesn't yet know where the avatar is. It learns the true position from a subsequent `ObjectUpdate` and then sends a second, correct `AgentUpdate`.
-   **`slv-rust` Implementation:** As noted in `rust_flow.md`, this problematic step is completely eliminated. `slv-rust` fetches capabilities via HTTPS first, which provides the correct avatar position. Therefore, the first `AgentUpdate` it sends in `circuit::advance_handshake` is already correct.

    **Impact:** This is a positive deviation that improves the robustness of the login process, but it is a deviation nonetheless. The implementation does not follow the traditional (and flawed) sequence of sending two `AgentUpdate` packets.

## Summary of Deviations

| Feature/Step             | Official Flow Description                                  | `slv-rust` Implementation                                    | Type of Deviation      |
| ------------------------ | ---------------------------------------------------------- | ------------------------------------------------------------ | ---------------------- |
| `UuidNameRequest`        | Sent early in UDP handshake.                               | **Not implemented.**                                         | Omission (Optimization) |
| `EventQueueGet`          | Polling starts during the handshake.                       | **Deferred** to be started by the UI layer after login.      | Architectural Change   |
| `RegionHandshakeReply` Delay | A 2-second delay is recommended.                           | **No delay** is implemented.                                 | Omission (Legacy)      |
| First `AgentUpdate`      | Sent with a bogus position.                                | Sent with the **correct position** (obtained from capabilities). | Improvement / Change   |

These deviations reflect a conscious effort in `slv-rust` to modernize the login protocol, prioritize performance, and avoid legacy workarounds. However, they are important to document for anyone comparing the implementation to the official specification.
