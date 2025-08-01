# ADR-0002: Networking Protocol Choice

**Date**: 2025-07-30

**Status**: Accepted

## Context

The SecondLife protocol is built on top of UDP, which provides low-latency communication but lacks the reliability and connection management features of TCP. A key architectural decision for `slv-rust` is how to handle the transport layer for SecondLife's UDP-based protocol. We need a solution that is performant, reliable, and provides modern features like encryption and congestion control. For a detailed technical analysis of the Second Life protocol, refer to `secondlife-protocol-technical-analysis.md`.

The options are:
1.  Implement a custom reliability layer on top of raw UDP.
2.  Use an existing library that provides a reliable transport protocol over UDP.

## Decision

We have decided to use `quinn`, a Rust implementation of the QUIC protocol, as the primary transport for our networking layer. We will also maintain the ability to fall back to raw UDP if necessary.

### Rationale

#### `quinn` for a Modern Transport Layer

-   **Reliability and Congestion Control**: QUIC provides TCP-like reliability, including packet retransmission and congestion control, but over UDP. This means we get the benefits of a reliable protocol without having to implement it ourselves.
-   **Security**: QUIC has built-in TLS 1.3 encryption, which provides a secure communication channel by default. This is a significant improvement over the unencrypted nature of the original SecondLife protocol.
-   **Performance**: QUIC is designed for low-latency communication. It features reduced head-of-line blocking compared to TCP, which is beneficial for real-time applications like SecondLife.
-   **Multiplexing**: QUIC supports multiple streams over a single connection, which can be used to separate different types of data (e.g., object updates, chat messages, asset transfers) and prevent them from blocking each other.

#### Fallback to Raw UDP

-   **Compatibility**: While QUIC is a superior protocol, some network environments or older SecondLife server versions may not support it. Maintaining a raw UDP transport allows for maximum compatibility.
-   **Performance Testing**: Having a raw UDP implementation allows us to benchmark the performance of `quinn` against a baseline and ensure that the overhead of QUIC is acceptable.

## Consequences

### Positive

-   We get a modern, secure, and reliable transport protocol with minimal implementation effort.
-   The networking layer will be more robust and performant than a custom solution built on raw UDP.
-   We maintain compatibility with a wide range of network environments.

### Negative

-   `quinn` adds a dependency to the project and introduces its own set of abstractions and potential complexities.
-   The overhead of QUIC might be slightly higher than a highly-optimized, custom UDP reliability layer, but the benefits are expected to outweigh this cost.

## Alternatives Considered

-   **`laminar`**: A lightweight, message-based networking library for games. While it is a good option, `quinn` provides a more complete and standardized transport protocol.
-   **Custom Implementation**: We could have implemented our own reliability layer on top of `tokio::net::UdpSocket`. This would have given us maximum control but would also have been a significant amount of work and a potential source of bugs. Using a well-tested library like `quinn` is a more pragmatic approach.
