# Phase 4: Application Integration & Message Handling

## Objective
To define and implement a clean, decoupled API between the networking layer and the rest of the application (UI, world state, etc.). This ensures that modules can communicate without having tight dependencies, making the codebase more modular and easier to maintain.

## Core Strategy: Asynchronous Channels
The primary mechanism for communication will be `tokio::sync::mpsc` channels. The networking layer will run in its own set of Tokio tasks and communicate with the main application thread (and its sub-modules) via these channels.

## Key Architectural Components

### 1. Network Command Channel (App -> Net)
A single channel will be used for sending commands *from* the application *to* the networking layer.

*   **Action:** Define a `NetworkCommand` enum.
    ```rust
    // In a new file, e.g., src/networking/commands.rs
    pub enum NetworkCommand {
        SendChat { message: String, channel: i32, chat_type: u8 },
        SendAgentUpdate { /* ... agent state ... */ },
        RequestObject { id: u32 },
        // ... other commands
    }
    ```
*   **Implementation:**
    *   The main application struct (`App` in `app.rs`) will hold the `mpsc::Sender<NetworkCommand>`.
    *   UI components (like the chat window) will call methods on `App` which, in turn, will create and send the appropriate `NetworkCommand`.
    *   The `Circuit`'s main task loop will have the `mpsc::Receiver<NetworkCommand>`. It will listen for commands, construct the appropriate auto-generated `Message` struct, and send it using the `Circuit::send_message` method from Phase 3.

### 2. Event/Data Channels (Net -> App)
Multiple channels will be used for sending events and data *from* the networking layer *to* the application. This allows different parts of the app to subscribe only to the data they need.

*   **Action:** Define structs for the data being passed. These should be clean, self-contained data structures, separate from the raw protocol messages.
    ```rust
    // In a new file, e.g., src/world/events.rs
    pub struct ChatEvent {
        pub sender_name: String,
        pub message: String,
        // ...
    }

    pub struct ObjectUpdateEvent {
        pub id: u32,
        pub position: glam::Vec3,
        // ...
    }
    ```
*   **Implementation:**
    *   The `App` struct will create and hold the `mpsc::Receiver` ends of these channels.
    *   The `Circuit` will hold the `mpsc::Sender` ends.
    *   In `circuit.rs`, the `handle_incoming_message` function will be the central dispatcher. It will `match` on the incoming, decoded `Message` enum.
    *   Based on the message type, it will transform the data from the raw protocol struct into a clean event struct and send it over the appropriate channel.
        ```rust
        // In circuit.rs's message handling loop
        match message {
            Message::ChatFromSimulator(data) => {
                let event = ChatEvent {
                    sender_name: String::from_utf8_lossy(&data.FromName).to_string(),
                    message: String::from_utf8_lossy(&data.Message).to_string(),
                };
                self.chat_event_sender.send(event).await;
            }
            Message::ObjectUpdate(data) => {
                // ... create and send ObjectUpdateEvent
            }
            // ... other message types
        }
        ```
    *   The main application loop in `app.rs` or `main_window.rs` will poll the receiver ends of these channels each frame/tick and update the application state accordingly (e.g., add a line to the chat UI, update an object's position in the world renderer).

## Files to Modify
*   `src/app.rs`: To own the channels and manage the top-level state updates.
*   `src/networking/circuit.rs`: To hold the sender ends of the event channels and implement the dispatch logic.
*   `src/ui/main_window.rs`, `src/ui/chat.rs`, etc.: To receive events from the `App` and update the UI, and to send commands to the `App`.
*   `src/world/mod.rs`: To receive world-related events (like object updates) and update the scene graph.

This architecture ensures that the networking code is solely responsible for communication and protocol details, while the rest of the application deals with clean, application-specific data structures, unaware of the underlying LLUDP protocol.
