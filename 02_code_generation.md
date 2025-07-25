# Phase 2: Automated Code Generation

## Objective
Create a `build.rs` script that leverages the parser from Phase 1 to automatically generate the Rust source code for all message structs and their codecs. This eliminates the need for manual, error-prone implementation of the protocol.

## File Path
*   `build.rs` (at the project root)

## Dependencies
The build script will need to add the following to `[build-dependencies]` in `Cargo.toml`:
*   `anyhow` (for error handling)
*   Potentially a code-formatting tool like `prettyplease` to make the generated code readable.

## Process Flow
The `build.rs` script will execute the following steps at compile time:

1.  **Locate Template:** Find and read the `/Users/dextro/RubymineProjects/slv-rust/message_template.msg` file.
2.  **Parse Template:** Invoke the `template_parser::parse()` function from Phase 1 to get the structured `MessageTemplate` data.
3.  **Generate Code:** Iterate through the `MessageTemplate` data structure and generate the raw string content for two separate Rust files.
4.  **Write to `OUT_DIR`:** Write the generated strings to `messages.rs` and `codecs.rs` inside the directory specified by the `OUT_DIR` environment variable.
5.  **Re-run Instruction:** The script must instruct Cargo to re-run it if `message_template.msg` or the build script itself changes.

## Generated Files

### 1. `messages.rs`
*   **Purpose:** To define all the message-related data structures.
*   **Contents:**
    *   An `#[derive(Debug, Clone)]` enum named `Message` with a variant for every message in the template.
    *   For each message, a corresponding `#[derive(Debug, Clone)]` struct will be generated (e.g., `pub struct TestMessage { ... }`). The `Message` enum variant will contain this struct.
    *   Fields within the generated structs will have their types mapped from the template to Rust types (see **Type Mapping** below).
    *   The file will also contain generated structs for each `Block` in the template.

### 2. `codecs.rs`
*   **Purpose:** To define the serialization and deserialization logic.
*   **Contents:**
    *   A `pub struct MessageCodec;`
    *   An `impl MessageCodec` block with two primary functions:
        *   `pub fn decode(bytes: &[u8]) -> Result<(PacketHeader, Message), Error>`: This function will contain a large `match` statement on the message ID. Each `match` arm will contain the specific logic to parse the bytes for that message and construct the corresponding `Message` enum variant.
        *   `pub fn encode(message: &Message, buffer: &mut Vec<u8>) -> Result<(), Error>`: This function will contain a `match` statement on the `Message` enum variant. Each arm will serialize the fields of the message struct into the provided byte buffer.

## Type Mapping
The build script must implement a mapping from the template's type names to Rust types.

| Template Type  | Rust Type                  | Notes                               |
|----------------|----------------------------|-------------------------------------|
| `U8`           | `u8`                       |                                     |
| `U16`          | `u16`                      |                                     |
| `U32`          | `u32`                      |                                     |
| `U64`          | `u64`                      |                                     |
| `S8`           | `i8`                       |                                     |
| `S16`          | `i16`                      |                                     |
| `S32`          | `i32`                      |                                     |
| `S64`          | `i64`                      |                                     |
| `F32`          | `f32`                      |                                     |
| `F64`          | `f64`                      |                                     |
| `LLUUID`       | `uuid::Uuid`               |                                     |
| `BOOL`         | `bool`                     |                                     |
| `IPADDR`       | `std::net::Ipv4Addr`       | Stored as a u32.                  |
| `IPPORT`       | `u16`                      |                                     |
| `LLVector3`    | `glam::Vec3`               |                                     |
| `LLVector3d`   | `glam::DVec3`              |                                     |
| `LLQuaternion` | `glam::Quat`               |                                     |
| `Variable 1`   | `Vec<u8>`                  | Prepended with a `u8` length.       |
| `Variable 2`   | `Vec<u8>`                  | Prepended with a `u16` length.      |
| `Fixed`        | `[u8; N]`                  | Where N is the block count.         |

## Integration
The main crate will include the generated code by placing the following in `src/networking/protocol/mod.rs`:

```rust
// This will be inside the protocol module
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub mod codecs {
    // Important: bring the generated messages into scope for the codec
    use super::messages::*;
    include!(concat!(env!("OUT_DIR"), "/codecs.rs"));
}
```
This ensures the rest of the application can use the generated code seamlessly.
