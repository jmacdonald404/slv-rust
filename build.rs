use anyhow::{Result, anyhow};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::collections::HashSet;

mod template_parser {
    include!("src/utils/build_utils/template_parser.rs");
}

use template_parser::{MessageDefinition, BlockDefinition, Cardinality};

fn generate_block_structs(code: &mut String, message: &MessageDefinition, generated_blocks: &mut HashSet<String>) {
    for block in &message.blocks {
        if matches!(block.cardinality, Cardinality::Multiple | Cardinality::Variable) && !block.fields.is_empty() {
            let block_name = format!("{}Block", block.name);
            
            // Skip if we've already generated this block struct
            if generated_blocks.contains(&block_name) {
                continue;
            }
            generated_blocks.insert(block_name.clone());
            
            code.push_str(&format!("#[derive(Debug, Clone, PartialEq)]\n"));
            code.push_str(&format!("pub struct {} {{\n", block_name));
            
            for field in &block.fields {
                let rust_type = map_field_type(&field.type_name);
                code.push_str(&format!("    pub {}: {},\n", 
                    to_snake_case(&field.name), rust_type));
            }
            
            code.push_str("}\n\n");
            
            // Generate placeholder Encode implementation for block
            code.push_str(&format!("impl Encode for {} {{\n", block_name));
            code.push_str("    fn encode<W: Write>(&self, _writer: &mut W) -> Result<()> {\n");
            code.push_str("        // TODO: Implement block encoding logic\n");
            code.push_str("        Ok(())\n");
            code.push_str("    }\n");
            code.push_str("}\n\n");
            
            // Generate placeholder Decode implementation for block
            code.push_str(&format!("impl Decode for {} {{\n", block_name));
            code.push_str("    fn decode<R: Read>(_reader: &mut R) -> Result<Self> {\n");
            code.push_str("        // TODO: Implement block decoding logic\n");
            code.push_str("        anyhow::bail!(\"Block decoding not yet implemented\")\n");
            code.push_str("    }\n");
            code.push_str("}\n\n");
        }
    }
}

fn generate_message_struct(code: &mut String, message: &MessageDefinition) {
    code.push_str(&format!("#[derive(Debug, Clone, PartialEq)]\n"));
    code.push_str(&format!("pub struct {} {{\n", message.name));
    
    // Track field names to detect and resolve conflicts
    let mut field_names = std::collections::HashSet::new();
    let mut field_counter = std::collections::HashMap::new();
    
    for block in &message.blocks {
        generate_block_fields_with_dedup(code, block, &mut field_names, &mut field_counter);
    }
    
    code.push_str("}\n\n");
    
    // Generate placeholder Encode implementation 
    code.push_str(&format!("impl Encode for {} {{\n", message.name));
    code.push_str("    fn encode<W: Write>(&self, _writer: &mut W) -> Result<()> {\n");
    code.push_str("        // TODO: Implement proper SL protocol encoding\n");
    code.push_str("        Ok(())\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // Generate placeholder Decode implementation
    code.push_str(&format!("impl Decode for {} {{\n", message.name));
    code.push_str("    fn decode<R: Read>(_reader: &mut R) -> Result<Self> {\n");
    code.push_str("        // TODO: Implement proper SL protocol decoding\n");
    code.push_str("        anyhow::bail!(\"Message decoding not yet implemented\")\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
}


fn generate_block_fields_with_dedup(
    code: &mut String, 
    block: &BlockDefinition, 
    field_names: &mut std::collections::HashSet<String>,
    field_counter: &mut std::collections::HashMap<String, u32>
) {
    match block.cardinality {
        Cardinality::Single => {
            // Single block - generate fields directly with deduplication
            for field in &block.fields {
                let rust_type = map_field_type(&field.type_name);
                let mut field_name = to_snake_case(&field.name);
                
                // Handle field name conflicts by appending counter
                if field_names.contains(&field_name) {
                    let counter = field_counter.entry(field_name.clone()).or_insert(1);
                    *counter += 1;
                    field_name = format!("{}_{}", field_name, counter);
                }
                
                field_names.insert(field_name.clone());
                code.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
            }
        }
        Cardinality::Multiple => {
            // Multiple blocks - wrap in Vec
            if !block.fields.is_empty() {
                let mut block_field_name = to_snake_case(&block.name);
                
                // Handle block name conflicts
                if field_names.contains(&block_field_name) {
                    let counter = field_counter.entry(block_field_name.clone()).or_insert(1);
                    *counter += 1;
                    block_field_name = format!("{}_{}", block_field_name, counter);
                }
                
                field_names.insert(block_field_name.clone());
                code.push_str(&format!("    pub {}: Vec<{}Block>,\n", 
                    block_field_name, block.name));
            }
        }
        Cardinality::Variable => {
            // Variable blocks - also wrap in Vec
            if !block.fields.is_empty() {
                let mut block_field_name = to_snake_case(&block.name);
                
                // Handle block name conflicts
                if field_names.contains(&block_field_name) {
                    let counter = field_counter.entry(block_field_name.clone()).or_insert(1);
                    *counter += 1;
                    block_field_name = format!("{}_{}", block_field_name, counter);
                }
                
                field_names.insert(block_field_name.clone());
                code.push_str(&format!("    pub {}: Vec<{}Block>,\n", 
                    block_field_name, block.name));
            }
        }
    }
}


fn map_field_type(sl_type: &str) -> &'static str {
    match sl_type {
        "U8" => "u8",
        "U16" => "u16", 
        "U32" => "u32",
        "U64" => "u64",
        "S8" => "i8",
        "S16" => "i16",
        "S32" => "i32", 
        "S64" => "i64",
        "F32" => "f32",
        "F64" => "f64",
        "LLUUID" => "Uuid",
        "BOOL" => "bool",
        "LLVector3" => "[f32; 3]",
        "LLVector4" => "[f32; 4]",
        "LLQuaternion" => "[f32; 4]",
        _ if sl_type.starts_with("Variable") => "Vec<u8>",
        _ if sl_type.starts_with("Fixed") => "Vec<u8>",
        _ => "Vec<u8>", // Default fallback
    }
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper = false;
    
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !prev_was_upper {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
            prev_was_upper = true;
        } else {
            result.push(c);
            prev_was_upper = false;
        }
    }
    
    // Escape Rust keywords and reserved words
    match result.as_str() {
        "type" => "r#type".to_string(),
        "match" => "r#match".to_string(),
        "self" => "r#self".to_string(),
        "super" => "r#super".to_string(),
        "crate" => "r#crate".to_string(),
        "loop" => "r#loop".to_string(),
        "move" => "r#move".to_string(),
        "ref" => "r#ref".to_string(),
        "mut" => "r#mut".to_string(),
        "static" => "r#static".to_string(),
        "const" => "r#const".to_string(),
        "fn" => "r#fn".to_string(),
        "let" => "r#let".to_string(),
        "if" => "r#if".to_string(),
        "else" => "r#else".to_string(),
        "while" => "r#while".to_string(),
        "for" => "r#for".to_string(),
        "in" => "r#in".to_string(),
        "where" => "r#where".to_string(),
        "impl" => "r#impl".to_string(),
        "trait" => "r#trait".to_string(),
        "struct" => "r#struct".to_string(),
        "enum" => "r#enum".to_string(),
        "use" => "r#use".to_string(),
        "mod" => "r#mod".to_string(),
        "pub" => "r#pub".to_string(),
        "override" => "r#override".to_string(),
        "final" => "r#final".to_string(),
        "abstract" => "r#abstract".to_string(),
        "async" => "r#async".to_string(),
        "await" => "r#await".to_string(),
        "become" => "r#become".to_string(),
        "box" => "r#box".to_string(),
        "do" => "r#do".to_string(),
        "extern" => "r#extern".to_string(),
        "macro" => "r#macro".to_string(),
        "priv" => "r#priv".to_string(),
        "typeof" => "r#typeof".to_string(),
        "union" => "r#union".to_string(),
        "unsafe" => "r#unsafe".to_string(),
        "unsized" => "r#unsized".to_string(),
        "virtual" => "r#virtual".to_string(),
        "yield" => "r#yield".to_string(),
        "try" => "r#try".to_string(),
        _ => result,
    }
}

fn main() -> Result<()> {
    // Ensure external/master-message-template is present and up-to-date
    let external_dir = Path::new("external");
    let repo_dir = external_dir.join("master-message-template");
    let repo_url = "https://github.com/secondlife/master-message-template";

    if !external_dir.exists() {
        fs::create_dir_all(&external_dir)?;
    }

    if !repo_dir.exists() {
        println!("Cloning master-message-template repository...");
        let status = Command::new("git")
            .args(["clone", repo_url, repo_dir.to_str().unwrap()])
            .status()
            .map_err(|e| anyhow!("Failed to execute git: {}", e))?;
        if !status.success() {
            return Err(anyhow!("git clone failed with status: {}", status));
        }
    } else {
        println!("Pulling latest changes in master-message-template repository...");
        let status = Command::new("git")
            .arg("-C")
            .arg(repo_dir.to_str().unwrap())
            .args(["pull", "--rebase"])
            .status()
            .map_err(|e| anyhow!("Failed to execute git: {}", e))?;
        if !status.success() {
            return Err(anyhow!("git pull failed with status: {}", status));
        }
    }

    // After git clone/pull logic
    let template_path = repo_dir.join("message_template.msg");
    let template_content = std::fs::read_to_string(&template_path)
        .map_err(|e| anyhow!("Failed to read messages_template.msg: {}", e))?;
    let parsed = template_parser::parse(&template_content)
        .map_err(|e| anyhow!("Failed to parse messages_template.msg: {}", e))?;
    println!("Parsed {} messages from messages_template.msg", parsed.messages.len());

    println!("cargo:rerun-if-changed=message_template.msg");
    println!("cargo:rerun-if-changed=build.rs");
    
    // Generate real message structs and enums from parsed data
    let out_dir = env::var("OUT_DIR")?;
    
    let mut code = String::new();
    code.push_str("// Auto-generated from message_template.msg - DO NOT EDIT MANUALLY\n\n");
    code.push_str("use uuid::Uuid;\n");
    code.push_str("use anyhow::{Result, anyhow};\n");
    code.push_str("use std::io::{Read, Write};\n");
    code.push_str("use super::codecs::{Encode, Decode};\n\n");
    
    // Generate block structs first (for Multiple/Variable blocks)
    let mut generated_blocks = HashSet::new();
    for message in &parsed.messages {
        generate_block_structs(&mut code, message, &mut generated_blocks);
    }
    
    // Generate individual message structs
    for message in &parsed.messages {
        generate_message_struct(&mut code, message);
    }
    
    // Generate the main Message enum
    code.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    code.push_str("pub enum Message {\n");
    for message in &parsed.messages {
        code.push_str(&format!("    {}({}),\n", message.name, message.name));
    }
    code.push_str("}\n\n");
    
    // Generate placeholder Encode implementation for Message enum
    code.push_str("impl Encode for Message {\n");
    code.push_str("    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {\n");
    code.push_str("        match self {\n");
    for message in &parsed.messages {
        code.push_str(&format!("            Message::{}(msg) => msg.encode(writer),\n", message.name));
    }
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // Generate message ID constants for reference
    code.push_str("// Message ID constants for reference\n");
    for message in &parsed.messages {
        let id_value = match message.frequency {
            template_parser::Frequency::Fixed => format!("0x{:08X}", message.id),
            _ => message.id.to_string(),
        };
        code.push_str(&format!("pub const {}_ID: u32 = {};\n", 
            message.name.to_uppercase(), id_value));
    }
    code.push_str("\n");
    
    // Generate placeholder Decode implementation for Message enum
    code.push_str("impl Decode for Message {\n");
    code.push_str("    fn decode<R: Read>(_reader: &mut R) -> Result<Self> {\n");
    code.push_str("        // TODO: Implement proper SL protocol message parsing\n");
    code.push_str("        // The SL protocol uses complex packet headers, not simple message IDs\n");
    code.push_str("        anyhow::bail!(\"Message decoding not yet implemented - use SLMessageCodec instead\")\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    let messages_path = std::path::Path::new(&out_dir).join("messages.rs");
    std::fs::write(&messages_path, code)?;
    println!("Generated complete messages.rs with {} message structs and main Message enum", parsed.messages.len());
    
    // Generate a minimal codecs.rs that matches the new generated Message type
    let codecs_content = r#"// Auto-generated from message_template.msg - DO NOT EDIT MANUALLY

use super::messages::Message;
use anyhow::{Result, anyhow};
use std::io::{Read, Write};
use uuid::Uuid;

pub trait Encode {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()>;
}

pub trait Decode: Sized {
    fn decode<R: Read>(reader: &mut R) -> Result<Self>;
}

impl Encode for u8 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[*self])?;
        Ok(())
    }
}

impl Decode for u8 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

impl Encode for u16 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for u16 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 2];
        reader.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
}

impl Encode for u32 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for u32 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 4];
        reader.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
}

impl Encode for u64 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for u64 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 8];
        reader.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
}

impl Encode for i8 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[*self as u8])?;
        Ok(())
    }
}

impl Decode for i8 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0] as i8)
    }
}

impl Encode for i16 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for i16 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 2];
        reader.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }
}

impl Encode for i32 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for i32 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 4];
        reader.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }
}

impl Encode for i64 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for i64 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 8];
        reader.read_exact(&mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }
}

impl Encode for f32 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for f32 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 4];
        reader.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }
}

impl Encode for f64 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl Decode for f64 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 8];
        reader.read_exact(&mut buf)?;
        Ok(f64::from_le_bytes(buf))
    }
}

impl Encode for bool {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[if *self { 1 } else { 0 }])?;
        Ok(())
    }
}

impl Decode for bool {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0] != 0)
    }
}

impl Encode for Uuid {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(self.as_bytes())?;
        Ok(())
    }
}

impl Decode for Uuid {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; 16];
        reader.read_exact(&mut buf)?;
        Ok(Uuid::from_bytes(buf))
    }
}

impl Encode for Vec<u8> {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(self)?;
        Ok(())
    }
}

impl Decode for Vec<u8> {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

impl Encode for String {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(self.as_bytes())?;
        Ok(())
    }
}

impl Decode for String {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }
}

impl Encode for [f32; 3] {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        for val in self.iter() {
            val.encode(writer)?;
        }
        Ok(())
    }
}

impl Decode for [f32; 3] {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        Ok([f32::decode(reader)?, f32::decode(reader)?, f32::decode(reader)?])
    }
}

impl Encode for [f32; 4] {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        for val in self.iter() {
            val.encode(writer)?;
        }
        Ok(())
    }
}

impl Decode for [f32; 4] {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        Ok([f32::decode(reader)?, f32::decode(reader)?, f32::decode(reader)?, f32::decode(reader)?])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketHeader {
    pub sequence_id: u32,
    pub flags: u8,
}

impl Encode for PacketHeader {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[self.flags])?;
        writer.write_all(&self.sequence_id.to_be_bytes())?;
        writer.write_all(&[0x00])?; // offset, always 0
        Ok(())
    }
}

impl Decode for PacketHeader {
    fn decode<R: Read>(reader: &mut R) -> Result<Self> {
        let mut flags = [0; 1];
        reader.read_exact(&mut flags)?;
        
        let mut seq_bytes = [0; 4];
        reader.read_exact(&mut seq_bytes)?;
        
        let mut offset = [0; 1];
        reader.read_exact(&mut offset)?; // Skip offset byte
        
        Ok(PacketHeader {
            flags: flags[0],
            sequence_id: u32::from_be_bytes(seq_bytes),
        })
    }
}

pub struct MessageCodec;

impl MessageCodec {
    pub fn decode(bytes: &[u8]) -> Result<(PacketHeader, Message)> {
        if bytes.len() < 6 {
            return Err(anyhow!("Packet too short: {} bytes", bytes.len()));
        }
        
        let mut reader = std::io::Cursor::new(bytes);
        let header = PacketHeader::decode(&mut reader)?;
        let message = Message::decode(&mut reader)?;
        Ok((header, message))
    }
    
    pub fn encode(header: &PacketHeader, message: &Message) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        header.encode(&mut buffer)?;
        message.encode(&mut buffer)?;
        Ok(buffer)
    }
}
"#;
    
    let codecs_path = Path::new(&out_dir).join("codecs.rs");
    fs::write(&codecs_path, codecs_content)?;
    println!("Generated minimal codecs.rs");
    
    Ok(())
}