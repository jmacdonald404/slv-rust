use anyhow::{Result, anyhow};
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=message_template.msg");
    println!("cargo:rerun-if-changed=build.rs");
    
    // For now, generate a minimal stub to prove the build system works
    let out_dir = env::var("OUT_DIR")?;
    
    // Generate a minimal messages.rs
    let messages_content = r#"// Auto-generated from message_template.msg - DO NOT EDIT MANUALLY

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    // Basic messages for testing
    TestMessage(TestMessage),
    PacketAck(PacketAck),
    UseCircuitCode(UseCircuitCode),
    CompleteAgentMovement(CompleteAgentMovement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestMessage {
    pub test1: u32,
}

#[derive(Debug, Clone, PartialEq)]  
pub struct PacketAck {
    pub packets: Vec<PacketAckBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketAckBlock {
    pub id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseCircuitCode {
    pub code: u32,
    pub session_id: Uuid,
    pub agent_id: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteAgentMovement {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub circuit_code: u32,
}
"#;
    
    let messages_path = Path::new(&out_dir).join("messages.rs");
    fs::write(&messages_path, messages_content)?;
    println!("Generated minimal messages.rs");
    
    // Generate a minimal codecs.rs
    let codecs_content = r#"// Auto-generated from message_template.msg - DO NOT EDIT MANUALLY

use super::generated_messages::*;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, PartialEq)]
pub struct PacketHeader {
    pub sequence_id: u32,
    pub flags: u32,
}

pub struct MessageCodec;

impl MessageCodec {
    pub fn decode(bytes: &[u8]) -> Result<(PacketHeader, Message)> {
        if bytes.len() < 7 {
            return Err(anyhow!("Packet too short"));
        }
        
        let flags = bytes[0];
        let sequence_id = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        let header = PacketHeader { sequence_id, flags: flags as u32 };
        
        // For now, return a simple test message
        let message = Message::TestMessage(TestMessage { test1: 42 });
        
        Ok((header, message))
    }
    
    pub fn encode(_message: &Message, _buffer: &mut Vec<u8>) -> Result<()> {
        // TODO: Implement proper serialization
        Ok(())
    }
}
"#;
    
    let codecs_path = Path::new(&out_dir).join("codecs.rs");
    fs::write(&codecs_path, codecs_content)?;
    println!("Generated minimal codecs.rs");
    
    Ok(())
}