# Second Life Login Protocol: `UseCircuitCode` Message

This document outlines the binary structure of the `UseCircuitCode` UDP message, which is the initial message sent from a client to a Second Life simulator to establish a connection.

## Overview

The login process begins with the client sending a `UseCircuitCode` message to the simulator. This message contains the client's `CircuitCode`, `SessionID`, and `AgentID`. The simulator uses this information to authenticate the client and establish a secure communication channel.

## UDP Packet Structure

The `UseCircuitCode` message is sent as a UDP packet. The packet has a header followed by the message body.

### UDP Packet Header

All Second Life UDP packets share a common header structure.

| Field | Type | Size (bytes) | Description |
|---|---|---|---|
| Flags | `U8` | 1 | Packet flags. For `UseCircuitCode`, this is typically `0x00`. |
| Packet ID | `U32` | 4 | A unique identifier for the packet, in big-endian format. |
| Message Number | `U32` | 4 | The message number for `UseCircuitCode`, in little-endian format. The exact number can vary, but it's determined from the message template. |

### `UseCircuitCode` Message Body

The body of the `UseCircuitCode` message contains the following fields in this specific order:

| Field | Type | Size (bytes) | Description |
|---|---|---|---|
| `Code` | `U32` | 4 | The client's circuit code. |
| `SessionID` | `LLUUID` | 16 | The client's session ID. |
| `ID` | `LLUUID` | 16 | The client's agent ID. |

**Total size of `UseCircuitCode` message body: 36 bytes**

## Constructing and Sending the `UseCircuitCode` Message in Rust

Here is an example of how to construct and send a `UseCircuitCode` message in Rust:

```rust
use std::net::UdpSocket;
use uuid::Uuid;

const USE_CIRCUIT_CODE_MESSAGE_NUMBER: u32 = 6; // Example message number

#[repr(C, packed)]
struct UseCircuitCode {
    code: u32,
    session_id: [u8; 16],
    id: [u8; 16],
}

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let sim_address = "127.0.0.1:13000"; // Example simulator address

    let circuit_code: u32 = 12345; // Example circuit code
    let session_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    // 1. Packet Header
    let flags: u8 = 0x00;
    let packet_id: u32 = 1; // Example packet ID

    let mut packet = Vec::new();
    packet.push(flags);
    packet.extend_from_slice(&packet_id.to_be_bytes());
    packet.extend_from_slice(&USE_CIRCUIT_CODE_MESSAGE_NUMBER.to_le_bytes());


    // 2. Message Body
    let use_circuit_code_msg = UseCircuitCode {
        code: circuit_code.to_le(),
        session_id: *session_id.as_bytes(),
        id: *agent_id.as_bytes(),
    };

    // Manually serialize the struct
    let mut message_body = Vec::new();
    message_body.extend_from_slice(&use_circuit_code_msg.code.to_le_bytes());
    message_body.extend_from_slice(&use_circuit_code_msg.session_id);
    message_body.extend_from_slice(&use_circuit_code_msg.id);

    packet.extend_from_slice(&message_body);


    // 3. Send the packet
    socket.send_to(&packet, sim_address)?;

    println!("UseCircuitCode message sent to {}", sim_address);

    Ok(())
}
```

**Note:** The `USE_CIRCUIT_CODE_MESSAGE_NUMBER` is an example. The actual number is dynamically assigned based on the `message_template.msg` file used by the server. You will need to determine the correct message number for the specific simulator you are connecting to.