# Networking Layer Implementation Plan

This document outlines the high-level plan to complete the networking layer for the `slv-rust` viewer. The core principle of this plan is to build a robust, maintainable, and accurate implementation by using the `message_template.msg` file as the single source of truth for the Second Life protocol.

The implementation is broken down into five major phases, each with its own detailed plan.

## Implementation Phases

1.  **Protocol Definition Parsing**
    *   **Objective:** Create a library module that can parse the `message_template.msg` file into a structured, in-memory representation. This is the foundation for all subsequent steps.
    *   **Details:** [01_protocol_parser.md](./01_protocol_parser.md)

2.  **Automated Code Generation**
    *   **Objective:** Implement a `build.rs` script that uses the protocol parser from Phase 1 to automatically generate the Rust source code for all message definitions and their corresponding serialization/deserialization logic (codecs).
    *   **Details:** [02_code_generation.md](./02_code_generation.md)

3.  **Connection & State Management**
    *   **Objective:** Refactor and enhance the existing `Circuit` and `UdpTransport` modules to use the auto-generated message code. This phase focuses on managing the UDP connection state, reliability, and the handshake sequence.
    *   **Details:** [03_connection_management.md](./03_connection_management.md)

4.  **Application Integration & Message Handling**
    *   **Objective:** Define and implement the API between the networking layer and the rest of the application (e.g., UI, world rendering). This will be achieved using a message-passing system (channels) to ensure loose coupling.
    *   **Details:** [04_application_integration.md](./04_application_integration.md)

5.  **Testing Strategy**
    *   **Objective:** Define a comprehensive testing strategy to ensure the correctness and robustness of the networking layer, from low-level parsing to full end-to-end integration tests.
    *   **Details:** [05_testing_strategy.md](./05_testing_strategy.md)
