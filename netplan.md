Document: Remaining Networking Implementation for slv-rust

1. Introduction

This document details the remaining tasks to complete the networking stack for the slv-rust Second Life viewer. The goal is to achieve a robust, performant, and feature-complete implementation that aligns with the protocol specifications outlined in secondlife-protocol-technical-analysis.md and the architectural decisions recorded in the project's documentation.

The current implementation has successfully established a foundational framework, including:

    An XML-RPC login system (src/networking/auth/).

    A circuit-based communication model for simulators (src/networking/circuit.rs).

    A packet generation system via build.rs and message_template.msg.

    A preliminary transport layer with a move towards QUIC (src/networking/quic_transport.rs).

    Initial packet handlers for core functionalities.

The following sections outline the work remaining, focusing on finalizing the transport layer, expanding protocol coverage, and ensuring seamless integration with other application components.

2. Core Transport Layer: Finalizing QUIC Integration

The decision to use QUIC (as noted in ADR-0002-networking-protocol-choice.md) is strategic, but the implementation is not yet complete. The legacy UDP semantics must be fully and correctly mapped to the QUIC protocol.

    Task: Complete quic_transport.rs Implementation

        Action: The current quic_transport.rs is a skeleton. It needs to be fully built out to manage a QUIC connection endpoint using the quinn crate. This includes connection establishment, handling TLS configuration for connecting to SL servers, and managing connection termination.

        Integration: The NetworkManager (src/networking/manager.rs) and NetworkClient (src/networking/client.rs) must be refactored to use the QuicTransport as their primary transport mechanism, removing dependencies on raw UDP sockets.

    Task: Map Second Life Protocol Semantics to QUIC Streams and Datagrams

        Context: The Second Life protocol embeds reliability flags (Reliable, Resent) and sequencing into its UDP packets. QUIC provides these features natively. A direct mapping is required to leverage QUIC's advantages.

        Action:

            Reliable Packets: Packets marked Reliable in the SL protocol should be sent over QUIC's reliable, ordered streams. This offloads the burden of ACK tracking and resending from our Circuit logic to the QUIC protocol itself.

            Unreliable Packets: Packets that can tolerate loss (e.g., frequent AgentUpdate packets) should be sent using QUIC's unreliable datagrams. This is the most efficient transport for this type of traffic and mirrors the intent of the original protocol.

            Circuit Abstraction: The Circuit struct should be refactored. Its responsibility for managing ACKs and resends (ack_in, ack_out) can be largely removed, as QUIC will handle this. The Circuit should instead focus on managing the QUIC connection to a specific simulator and dispatching received data to the correct handlers.

3. Packet and Message Handling

While a packet processing pipeline exists, it lacks coverage for the vast majority of the Second Life protocol's messages.

    Task: Comprehensive Packet Handler Implementation

        Context: The current handlers in src/networking/handlers/ cover only the most basic interactions (e.g., login, some agent movements).

        Action: A systematic effort is needed to implement handlers for the remaining critical message families. This involves creating new functions within the existing handler modules (agent_handlers.rs, region_handlers.rs, etc.) or creating new modules. Key missing areas include:

            Object & Prim Management: ObjectUpdate, ObjectUpdateCompressed, KillObject. This is fundamental for rendering the world.

            Inventory: FetchInventoryDescendents, InventoryDescendents. This requires parsing inventory data and integrating with src/ui/inventory.rs.

            Asset Transfer: TransferRequest, TransferInfo, TransferPacket. These packets initiate the process for downloading assets like textures and meshes.

            Terrain: LayerData. This is necessary for src/world/terrain.rs to construct the ground mesh.

            Social & Communication: Chat messages (ChatFromSimulator), IMs, group notices, and friend status updates.

            Avatar Appearance: AvatarAppearance, WearablesUpdate.

    Task: Complete message_template.msg Definitions

        Context: The packet structs in src/networking/packets/generated.rs are created by build.rs from message_template.msg. This template is currently sparse.

        Action: The message_template.msg file must be populated with definitions for all packets that the client needs to send and receive. This is a prerequisite for writing the handlers mentioned above and for implementing client-side actions.

4. Application-Layer Protocols: Capabilities & Asset Transfer

A significant portion of modern Second Life functionality operates over an HTTP-based system called "Capabilities," which is currently unimplemented.

    Task: Implement the Capabilities System

        Context: After login, the server provides a map of URLs (Capabilities) for various services like fetching assets, managing inventory, or receiving events. This is an HTTP-based, long-polling event queue system.

        Action:

            HTTP Client: Integrate an asynchronous HTTP client like reqwest or hyper into the NetworkManager.

            Capabilities Handler: Create a new module (src/networking/capabilities.rs) to manage the list of capability URLs received from the LoginReply block.

            Event Queue (CAPS): Implement a long-polling loop to query the EventMessageQueueGet capability. This will provide asynchronous events (like incoming inventory offers or teleport lures) that are not sent over the UDP/QUIC channel. These events must be parsed and dispatched to the relevant application modules (e.g., UI, world state).

    Task: Build an Integrated Asset Transfer System

        Context: Assets (textures, meshes, wearables) are downloaded via a combination of UDP/QUIC messages and direct HTTP requests to a CDN, often using URLs provided by the Capabilities system.

        Action:

            Refactor assets/manager.rs: The AssetManager should not handle networking directly. It should place a request for an asset.

            Create AssetDownloader: A new networking component, let's call it AssetDownloader, should listen for these requests.

            Implement Download Logic: This downloader will use the TransferRequest packet to initiate a download via the simulator's UDP channel or use the appropriate Capability URL to fetch the asset via HTTP.

            Integrate with Cache: Once downloaded, the asset data should be passed to the assets/cache.rs for storage and use by the rendering engine.

5. State Management and Integration

The networking layer must be fully decoupled from the rest of the application to improve maintainability and clarity.

    Task: Implement Simulator Handover Logic

        Context: Moving between regions requires connecting to a new simulator while gracefully disconnecting from the old one. The EnableSimulator and CompleteAgentMovement packets govern this process.

        Action: Implement a state machine within the NetworkManager to handle region crossings. This involves:

            Receiving EnableSimulator for the new region.

            Establishing a new Circuit and QUIC connection to the new simulator's IP and port.

            Sending UseCircuitCode to the new simulator.

            Sending CompleteAgentMovement to the new simulator.

            Once confirmed, terminating the connection to the old simulator.

    Task: Refactor Packet Handlers to Use an Event Bus

        Context: Currently, handlers might be tempted to directly modify world state. The ARCHITECTURE.md advocates for a decoupled system. The src/world/events.rs file provides a starting point for this.

        Action: All packet handlers in src/networking/handlers/ must be refactored. Instead of directly interacting with UI or world objects, they should parse the incoming packet data and emit a strongly-typed WorldEvent. For example, receiving an ObjectUpdate packet should cause the handler to emit WorldEvent::ObjectUpdated { ... }. Other modules, like src/world/mod.rs and src/rendering/engine.rs, will subscribe to this event bus and apply the changes, ensuring the networking crate has no direct knowledge of the application's internal state.
