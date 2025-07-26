use anyhow::{Result, anyhow};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

mod template_parser {
    include!("src/utils/build_utils/template_parser.rs");
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
    
    // For now, generate a minimal stub to prove the build system works
    let out_dir = env::var("OUT_DIR")?;
    
    // Generate minimal Message enum that won't conflict with HandshakeMessage
    let mut code = String::new();
    code.push_str("// Auto-generated stub - minimal Message enum\n\n");
    code.push_str("use uuid::Uuid;\n");
    code.push_str("use anyhow::{Result, anyhow};\n");
    code.push_str("use std::io::{Read, Write};\n");
    code.push_str("use super::codecs::{Encode, Decode};\n\n");
    
    // For now, just create a placeholder enum to avoid conflicts
    code.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    code.push_str("pub enum Message {\n");
    code.push_str("    // Placeholder - real messages handled by HandshakeMessage\n");
    code.push_str("    Placeholder,\n");
    code.push_str("}\n\n");
    
    // Add minimal implementations
    code.push_str("impl Encode for Message {\n");
    code.push_str("    fn encode<W: Write>(&self, _writer: &mut W) -> Result<()> {\n");
    code.push_str("        Ok(())\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl Decode for Message {\n");
    code.push_str("    fn decode<R: Read>(_reader: &mut R) -> Result<Self> {\n");
    code.push_str("        Ok(Message::Placeholder)\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    let messages_path = std::path::Path::new(&out_dir).join("messages.rs");
    std::fs::write(&messages_path, code)?;
    println!("Generated minimal messages.rs stub with {} messages parsed", parsed.messages.len());
    
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