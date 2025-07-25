# Phase 1: Protocol Definition Parsing

## Objective
Create a self-contained Rust module responsible for parsing the `message_template.msg` file. The parser will transform the raw text of the template into a structured, in-memory representation that can be easily consumed by the code generation step in Phase 2.

## File Path
*   `src/networking/protocol/template_parser.rs`

## Key Data Structures
The parser will populate the following Rust structs, which must be defined within the module:

```rust
// Represents the entire parsed message_template.msg file
pub struct MessageTemplate {
    pub messages: Vec<MessageDefinition>,
}

// Represents a single message definition
pub struct MessageDefinition {
    pub name: String,
    pub frequency: Frequency,
    pub id: u32,
    pub trust: TrustLevel,
    pub encoding: Encoding,
    pub blocks: Vec<BlockDefinition>,
}

// Represents a block within a message (e.g., { AgentData Single ... })
pub struct BlockDefinition {
    pub name: String,
    pub cardinality: Cardinality,
    pub count: Option<u32>, // For 'Multiple' cardinality
    pub fields: Vec<FieldDefinition>,
}

// Represents a single field within a block (e.g., { AgentID LLUUID })
pub struct FieldDefinition {
    pub name: String,
    pub type_name: String, // e.g., "U32", "LLVector3", "Variable 1"
}

// Enums to represent the different properties of a message
pub enum Frequency { High, Medium, Low, Fixed }
pub enum TrustLevel { NotTrusted, Trusted }
pub enum Encoding { Unencoded, Zerocoded }
pub enum Cardinality { Single, Multiple, Variable }
```

## Parsing Logic
The implementation should follow these guidelines:
1.  **Input:** The main parsing function will take the string content of `message_template.msg` as input.
2.  **Structure:** A line-by-line iterator combined with a simple state machine is recommended. The parser needs to track its current context (e.g., top-level, inside a message definition, inside a block definition).
3.  **Comments:** Lines beginning with `//` should be ignored.
4.  **Message Header:** The parser must correctly identify and parse the message header line (e.g., `TestMessage Low 1 NotTrusted Zerocoded`). This includes parsing the message ID, which can be a hex literal (e.g., `0xFFFFFFFB`).
5.  **Blocks and Fields:** The parser must correctly handle the nested structure of blocks and fields, including the cardinality and count for each block.
6.  **Robustness:** The parser should be able to handle various whitespace arrangements and empty lines gracefully.

## Public API
The module should expose a single primary function:

```rust
/// Parses the content of a message_template.msg file.
///
/// # Arguments
/// * `content` - A string slice containing the entire content of the template file.
///
/// # Returns
/// A `Result` containing the parsed `MessageTemplate` or a `String` error.
pub fn parse(content: &str) -> Result<MessageTemplate, String>;
```

## Error Handling
The parser must return a descriptive `Err(String)` for any of the following conditions:
*   Malformed message or block header lines.
*   Unrecognized `Frequency`, `TrustLevel`, `Encoding`, or `Cardinality`.
*   Mismatched curly braces `{}`.
*   Any other syntax violation of the template format.
