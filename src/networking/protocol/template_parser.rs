use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct MessageTemplate {
    pub messages: Vec<MessageDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageDefinition {
    pub name: String,
    pub frequency: Frequency,
    pub id: u32,
    pub trust: TrustLevel,
    pub encoding: Encoding,
    pub flags: Vec<String>, // Additional flags like "UDPBlackListed"
    pub blocks: Vec<BlockDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDefinition {
    pub name: String,
    pub cardinality: Cardinality,
    pub count: Option<u32>, // For 'Multiple' cardinality
    pub fields: Vec<FieldDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDefinition {
    pub name: String,
    pub type_name: String, // e.g., "U32", "LLVector3", "Variable 1"
}

#[derive(Debug, Clone, PartialEq)]
pub enum Frequency {
    High,
    Medium,
    Low,
    Fixed,
}

impl FromStr for Frequency {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "High" => Ok(Frequency::High),
            "Medium" => Ok(Frequency::Medium),
            "Low" => Ok(Frequency::Low),
            "Fixed" => Ok(Frequency::Fixed),
            _ => Err(format!("Unknown frequency: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    NotTrusted,
    Trusted,
}

impl FromStr for TrustLevel {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NotTrusted" => Ok(TrustLevel::NotTrusted),
            "Trusted" => Ok(TrustLevel::Trusted),
            _ => Err(format!("Unknown trust level: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Encoding {
    Unencoded,
    Zerocoded,
}

impl FromStr for Encoding {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Unencoded" => Ok(Encoding::Unencoded),
            "Zerocoded" => Ok(Encoding::Zerocoded),
            _ => Err(format!("Unknown encoding: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    Single,
    Multiple,
    Variable,
}

impl FromStr for Cardinality {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Single" => Ok(Cardinality::Single),
            "Multiple" => Ok(Cardinality::Multiple),
            "Variable" => Ok(Cardinality::Variable),
            _ => Err(format!("Unknown cardinality: {}", s)),
        }
    }
}

#[derive(Debug, PartialEq)]
enum ParseState {
    TopLevel,
    InMessage,
    InBlock,
}

/// Parses the content of a message_template.msg file.
///
/// # Arguments
/// * `content` - A string slice containing the entire content of the template file.
///
/// # Returns
/// A `Result` containing the parsed `MessageTemplate` or a `String` error.
pub fn parse(content: &str) -> Result<MessageTemplate, String> {
    let mut messages = Vec::new();
    let mut state = ParseState::TopLevel;
    let mut current_message: Option<MessageDefinition> = None;
    let mut current_block: Option<BlockDefinition> = None;
    let mut brace_depth = 0;
    
    let lines: Vec<&str> = content.lines().collect();
    let mut line_num = 0;
    
    while line_num < lines.len() {
        let line = lines[line_num].trim();
        line_num += 1;
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") || line.starts_with("version") {
            continue;
        }
        
        match state {
            ParseState::TopLevel => {
                if line == "{" {
                    // Start of a new message definition
                    brace_depth += 1;
                    state = ParseState::InMessage;
                } else if !line.is_empty() {
                    return Err(format!("Unexpected content at line {}: {}", line_num, line));
                }
            }
            
            ParseState::InMessage => {
                if line == "{" {
                    brace_depth += 1;
                    state = ParseState::InBlock;
                } else if line == "}" {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        // End of message definition
                        if let Some(message) = current_message.take() {
                            messages.push(message);
                        }
                        state = ParseState::TopLevel;
                    }
                } else {
                    // Parse message header line
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 5 {
                        return Err(format!("Invalid message header at line {}: expected at least 5 parts, got {}", line_num, parts.len()));
                    }
                    
                    let name = parts[0].to_string();
                    let frequency = parts[1].parse::<Frequency>()
                        .map_err(|e| format!("Error parsing frequency at line {}: {}", line_num, e))?;
                    
                    // Parse message ID, which can be hex (e.g., 0xFFFFFFFB) or decimal
                    let id = if parts[2].starts_with("0x") || parts[2].starts_with("0X") {
                        u32::from_str_radix(&parts[2][2..], 16)
                            .map_err(|e| format!("Error parsing hex message ID at line {}: {}", line_num, e))?
                    } else {
                        parts[2].parse::<u32>()
                            .map_err(|e| format!("Error parsing message ID at line {}: {}", line_num, e))?
                    };
                    
                    let trust = parts[3].parse::<TrustLevel>()
                        .map_err(|e| format!("Error parsing trust level at line {}: {}", line_num, e))?;
                    let encoding = parts[4].parse::<Encoding>()
                        .map_err(|e| format!("Error parsing encoding at line {}: {}", line_num, e))?;
                    
                    // Collect any additional flags (like "UDPBlackListed")
                    let flags = if parts.len() > 5 {
                        parts[5..].iter().map(|s| s.to_string()).collect()
                    } else {
                        Vec::new()
                    };
                    
                    current_message = Some(MessageDefinition {
                        name,
                        frequency,
                        id,
                        trust,
                        encoding,
                        flags,
                        blocks: Vec::new(),
                    });
                }
            }
            
            ParseState::InBlock => {
                if line == "{" {
                    brace_depth += 1;
                    // Start of field definitions within a block
                } else if line == "}" {
                    brace_depth -= 1;
                    if brace_depth == 1 {
                        // End of block definition
                        if let (Some(ref mut message), Some(block)) = (&mut current_message, current_block.take()) {
                            message.blocks.push(block);
                        }
                        state = ParseState::InMessage;
                    }
                } else if line.contains("{") && line.contains("}") {
                    // Field definition line like "{ Test1 U32 }"
                    let field_content = line.trim_start_matches('{').trim_end_matches('}').trim();
                    let field_parts: Vec<&str> = field_content.split_whitespace().collect();
                    
                    if field_parts.len() >= 2 {
                        let field_name = field_parts[0].to_string();
                        let field_type = field_parts[1..].join(" ");
                        
                        if let Some(ref mut block) = current_block {
                            block.fields.push(FieldDefinition {
                                name: field_name,
                                type_name: field_type,
                            });
                        }
                    }
                } else if !line.starts_with("{") && !line.ends_with("}") {
                    // Block header line like "TestBlock1 Single" or "NeighborBlock Multiple 4"
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 2 {
                        return Err(format!("Invalid block header at line {}: expected at least 2 parts", line_num));
                    }
                    
                    let block_name = parts[0].to_string();
                    let cardinality = parts[1].parse::<Cardinality>()
                        .map_err(|e| format!("Error parsing cardinality at line {}: {}", line_num, e))?;
                    
                    let count = if cardinality == Cardinality::Multiple && parts.len() > 2 {
                        Some(parts[2].parse::<u32>()
                            .map_err(|e| format!("Error parsing block count at line {}: {}", line_num, e))?)
                    } else {
                        None
                    };
                    
                    current_block = Some(BlockDefinition {
                        name: block_name,
                        cardinality,
                        count,
                        fields: Vec::new(),
                    });
                }
            }
        }
    }
    
    // Check for unmatched braces
    if brace_depth != 0 {
        return Err(format!("Unmatched braces: depth {} at end of file", brace_depth));
    }
    
    Ok(MessageTemplate { messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let input = r#"
{
    TestMessage Low 1 NotTrusted Zerocoded
    {
        TestBlock1 Single
        {   Test1   U32 }
    }
}
"#;
        
        let result = parse(input).unwrap();
        assert_eq!(result.messages.len(), 1);
        
        let message = &result.messages[0];
        assert_eq!(message.name, "TestMessage");
        assert_eq!(message.frequency, Frequency::Low);
        assert_eq!(message.id, 1);
        assert_eq!(message.trust, TrustLevel::NotTrusted);
        assert_eq!(message.encoding, Encoding::Zerocoded);
        assert_eq!(message.flags, Vec::<String>::new());
        assert_eq!(message.blocks.len(), 1);
        
        let block = &message.blocks[0];
        assert_eq!(block.name, "TestBlock1");
        assert_eq!(block.cardinality, Cardinality::Single);
        assert_eq!(block.count, None);
        assert_eq!(block.fields.len(), 1);
        
        let field = &block.fields[0];
        assert_eq!(field.name, "Test1");
        assert_eq!(field.type_name, "U32");
    }

    #[test]
    fn test_parse_hex_message_id() {
        let input = r#"
{
    PacketAck Fixed 0xFFFFFFFB NotTrusted Unencoded
    {
        Packets Variable
        {   ID  U32 }
    }
}
"#;
        
        let result = parse(input).unwrap();
        assert_eq!(result.messages.len(), 1);
        
        let message = &result.messages[0];
        assert_eq!(message.name, "PacketAck");
        assert_eq!(message.frequency, Frequency::Fixed);
        assert_eq!(message.id, 0xFFFFFFFB);
        assert_eq!(message.flags, Vec::<String>::new());
    }

    #[test]
    fn test_parse_multiple_cardinality() {
        let input = r#"
{
    TestMessage Low 1 NotTrusted Zerocoded
    {
        NeighborBlock Multiple 4
        {   Test0   U32 }
        {   Test1   U32 }
    }
}
"#;
        
        let result = parse(input).unwrap();
        let block = &result.messages[0].blocks[0];
        assert_eq!(block.cardinality, Cardinality::Multiple);
        assert_eq!(block.count, Some(4));
        assert_eq!(block.fields.len(), 2);
    }

    #[test]
    fn test_parse_error_invalid_frequency() {
        let input = r#"
{
    TestMessage InvalidFreq 1 NotTrusted Zerocoded
    {
        TestBlock1 Single
        {   Test1   U32 }
    }
}
"#;
        
        let result = parse(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown frequency"));
    }

    #[test]
    fn test_parse_message_with_flags() {
        let input = r#"
{
    OpenCircuit Fixed 0xFFFFFFFC NotTrusted Unencoded UDPBlackListed
    {
        CircuitInfo Single
        {   IP      IPADDR  }
        {   Port    IPPORT  }
    }
}
"#;
        
        let result = parse(input).unwrap();
        let message = &result.messages[0];
        assert_eq!(message.name, "OpenCircuit");
        assert_eq!(message.frequency, Frequency::Fixed);
        assert_eq!(message.id, 0xFFFFFFFC);
        assert_eq!(message.trust, TrustLevel::NotTrusted);
        assert_eq!(message.encoding, Encoding::Unencoded);
        assert_eq!(message.flags, vec!["UDPBlackListed".to_string()]);
    }

    #[test]
    fn test_parse_comments_ignored() {
        let input = r#"
// This is a comment
version 2.0
// Another comment

{
    TestMessage Low 1 NotTrusted Zerocoded
    {
        TestBlock1 Single
        {   Test1   U32 }
    }
}
"#;
        
        let result = parse(input).unwrap();
        assert_eq!(result.messages.len(), 1);
    }

    #[test]
    fn test_parse_actual_message_template() {
        let content = std::fs::read_to_string("message_template.msg");
        if let Ok(content) = content {
            let result = parse(&content);
            match result {
                Ok(template) => {
                    println!("Successfully parsed {} messages", template.messages.len());
                    // Test a few specific messages we know should be there
                    let test_message = template.messages.iter().find(|m| m.name == "TestMessage");
                    assert!(test_message.is_some(), "TestMessage should be present");
                    
                    let packet_ack = template.messages.iter().find(|m| m.name == "PacketAck");
                    assert!(packet_ack.is_some(), "PacketAck should be present");
                    if let Some(ack) = packet_ack {
                        assert_eq!(ack.id, 0xFFFFFFFB);
                        assert_eq!(ack.frequency, Frequency::Fixed);
                    }
                }
                Err(e) => {
                    println!("Parse error: {}", e);
                    panic!("Failed to parse message_template.msg: {}", e);
                }
            }
        } else {
            println!("message_template.msg not found, skipping integration test");
        }
    }
}