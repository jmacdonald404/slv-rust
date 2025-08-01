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
            
            code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize)]
"));
            code.push_str(&format!("pub struct {} {{
", block_name));
            
            let mut field_names = std::collections::HashSet::new();
            let mut field_counter = std::collections::HashMap::new();

            for field in &block.fields {
                let rust_type = map_field_type(&field.type_name);
                let mut field_name = to_snake_case(&field.name);

                if field_names.contains(&field_name) {
                    let counter = field_counter.entry(field_name.clone()).or_insert(1);
                    *counter += 1;
                    field_name = format!("{}_{}", field_name, counter);
                }
                
                field_names.insert(field_name.clone());
                code.push_str(&format!("    pub {}: {},
", 
                    field_name, rust_type));
            }
            
            code.push_str("}

");
        }
    }
}

fn generate_message_struct(code: &mut String, message: &MessageDefinition) {
    code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize)]\n"));
    code.push_str(&format!("pub struct {} {{\n", message.name));
    
    // Track field names to detect and resolve conflicts
    let mut field_names = std::collections::HashSet::new();
    let mut field_counter = std::collections::HashMap::new();
    
    for block in &message.blocks {
        generate_block_fields_with_dedup(code, block, &mut field_names, &mut field_counter);
    }
    
    code.push_str("}\n\n");
    
    // Generate Packet trait implementation
    let frequency_str = match message.frequency {
        template_parser::Frequency::High => "PacketFrequency::High",
        template_parser::Frequency::Medium => "PacketFrequency::Medium", 
        template_parser::Frequency::Low => "PacketFrequency::Low",
        template_parser::Frequency::Fixed => "PacketFrequency::Fixed",
    };
    
    let trusted = match message.trust {
        template_parser::TrustLevel::Trusted => "true",
        template_parser::TrustLevel::NotTrusted => "false",
    };
    
    let zerocoded = match message.encoding {
        template_parser::Encoding::Zerocoded => "true",
        template_parser::Encoding::Unencoded => "false",
    };
    
    code.push_str(&format!("impl Packet for {} {{\n", message.name));
    code.push_str(&format!("    const ID: u32 = {};\n", message.id));
    code.push_str(&format!("    const FREQUENCY: PacketFrequency = {};\n", frequency_str));
    code.push_str("    const RELIABLE: bool = true;\n"); // Default to reliable
    code.push_str(&format!("    const ZEROCODED: bool = {};\n", zerocoded));
    code.push_str(&format!("    const TRUSTED: bool = {};\n", trusted));
    code.push_str("    \n");
    code.push_str(&format!("    fn name() -> &'static str {{ \"{}\" }}\n", message.name));
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
        "U8" => "U8",
        "U16" => "U16", 
        "U32" => "U32",
        "U64" => "u64",
        "S8" => "i8",
        "S16" => "i16",
        "S32" => "S32", 
        "S64" => "i64",
        "F32" => "F32",
        "F64" => "F64",
        "LLUUID" => "LLUUID",
        "BOOL" => "BOOL",
        "IPADDR" => "IPADDR",
        "IPPORT" => "IPPORT",
        "LLVector3" => "LLVector3",
        "LLVector4" => "LLVector4",
        "LLQuaternion" => "LLQuaternion",
        "Variable 1" => "LLVariable1",
        "Variable 2" => "LLVariable2",
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
    code.push_str("use super::{Packet, PacketFrequency};\n");
    code.push_str("use super::types::*;\n");
    code.push_str("use serde::{Deserialize, Serialize};\n\n");
    
    // Generate block structs first (for Multiple/Variable blocks)
    let mut generated_blocks = HashSet::new();
    for message in &parsed.messages {
        generate_block_structs(&mut code, message, &mut generated_blocks);
    }
    
    // Generate individual message structs
    for message in &parsed.messages {
        generate_message_struct(&mut code, message);
    }
    

    let messages_path = std::path::Path::new(&out_dir).join("messages.rs");
    std::fs::write(&messages_path, code)?;
    println!("Generated complete messages.rs with {} packet structs", parsed.messages.len());
    
    Ok(())
}