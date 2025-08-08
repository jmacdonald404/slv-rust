//! Template-based Second Life protocol deserializer
//! 
//! This module implements proper Second Life UDP packet deserialization that can handle
//! protocol evolution gracefully by parsing packets field-by-field according to message
//! templates, rather than expecting exact binary compatibility with Rust structs.

use crate::networking::{NetworkError, NetworkResult};
use crate::utils::build_utils::template_parser::{MessageDefinition, BlockDefinition, FieldDefinition, Cardinality};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use tracing::{debug, warn};

/// Represents a parsed field value from a packet
#[derive(Debug, Clone)]
pub enum FieldValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    S8(i8),
    S16(i16),
    S32(i32),
    S64(i64),
    F32(f32),
    F64(f64),
    LLUUID([u8; 16]),
    BOOL(bool),
    IPADDR([u8; 4]),
    IPPORT(u16),
    LLVector3([f32; 3]),
    LLVector3d([f64; 3]),
    LLVector4([f32; 4]),
    LLQuaternion([f32; 4]),
    Variable(Vec<u8>),
    Fixed(Vec<u8>),
}

impl FieldValue {
    pub fn as_u8(&self) -> NetworkResult<u8> {
        match self {
            FieldValue::U8(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected U8, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_bytes(&self) -> NetworkResult<&Vec<u8>> {
        match self {
            FieldValue::Variable(v) | FieldValue::Fixed(v) => Ok(v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected Variable/Fixed bytes, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_f32(&self) -> NetworkResult<f32> {
        match self {
            FieldValue::F32(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected F32, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_u32(&self) -> NetworkResult<u32> {
        match self {
            FieldValue::U32(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected U32, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_uuid_bytes(&self) -> NetworkResult<[u8; 16]> {
        match self {
            FieldValue::LLUUID(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected LLUUID, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_quaternion(&self) -> NetworkResult<[f32; 4]> {
        match self {
            FieldValue::LLQuaternion(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected LLQuaternion, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_vector3(&self) -> NetworkResult<[f32; 3]> {
        match self {
            FieldValue::LLVector3(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected LLVector3, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_u16(&self) -> NetworkResult<u16> {
        match self {
            FieldValue::U16(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected U16, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_u64(&self) -> NetworkResult<u64> {
        match self {
            FieldValue::U64(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected U64, got {:?}", self) 
            }),
        }
    }
    
    pub fn as_s16(&self) -> NetworkResult<i16> {
        match self {
            FieldValue::S16(v) => Ok(*v),
            _ => Err(NetworkError::PacketDecode { 
                reason: format!("Expected S16, got {:?}", self) 
            }),
        }
    }
}

/// Represents a parsed block containing fields
#[derive(Debug, Clone)]
pub struct ParsedBlock {
    pub name: String,
    pub fields: HashMap<String, FieldValue>,
}

impl ParsedBlock {
    pub fn new(name: String) -> Self {
        Self {
            name,
            fields: HashMap::new(),
        }
    }
    
    pub fn add_field(&mut self, name: String, value: FieldValue) {
        self.fields.insert(name, value);
    }
    
    pub fn get_field(&self, name: &str) -> NetworkResult<&FieldValue> {
        self.fields.get(name).ok_or_else(|| NetworkError::PacketDecode {
            reason: format!("Field '{}' not found in block '{}'", name, self.name),
        })
    }
}

/// Represents a fully parsed message
#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub name: String,
    pub blocks: HashMap<String, Vec<ParsedBlock>>,
}

impl ParsedMessage {
    pub fn new(name: String) -> Self {
        Self {
            name,
            blocks: HashMap::new(),
        }
    }
    
    pub fn add_block(&mut self, block: ParsedBlock) {
        self.blocks.entry(block.name.clone())
            .or_insert_with(Vec::new)
            .push(block);
    }
    
    pub fn get_single_block(&self, name: &str) -> NetworkResult<&ParsedBlock> {
        let blocks = self.blocks.get(name).ok_or_else(|| NetworkError::PacketDecode {
            reason: format!("Block '{}' not found in message '{}'", name, self.name),
        })?;
        
        if blocks.len() != 1 {
            return Err(NetworkError::PacketDecode {
                reason: format!("Expected single block '{}', found {}", name, blocks.len()),
            });
        }
        
        Ok(&blocks[0])
    }
}

/// Template-based packet deserializer
pub struct TemplateDeserializer;

impl TemplateDeserializer {
    pub fn new() -> Self {
        Self
    }
    
    /// Parse a packet using its message template
    pub fn parse_packet(&self, data: &[u8], template: &MessageDefinition) -> NetworkResult<ParsedMessage> {
        let mut reader = Cursor::new(data);
        let mut message = ParsedMessage::new(template.name.clone());
        
        debug!("Parsing {} packet with {} bytes", template.name, data.len());
        
        for block_template in &template.blocks {
            // Handle EOF gracefully - some blocks may be optional
            if reader.position() >= data.len() as u64 {
                debug!("Data ended before block '{}', stopping parse", block_template.name);
                break;
            }
            
            let repeat_count = self.get_block_repeat_count(&mut reader, block_template)?;
            debug!("Block '{}' repeat count: {}", block_template.name, repeat_count);
            
            for i in 0..repeat_count {
                // Check if we have enough data before parsing each block instance
                if reader.position() >= data.len() as u64 {
                    debug!("Data ended before block '{}' instance {}, stopping", block_template.name, i);
                    break;
                }
                
                let block = self.parse_block(&mut reader, block_template)?;
                message.add_block(block);
            }
        }
        
        // Warn about excess data but don't fail
        let remaining = data.len() as u64 - reader.position();
        if remaining > 0 {
            warn!("Packet '{}' has {} unread bytes - protocol template may be outdated", 
                  template.name, remaining);
        }
        
        Ok(message)
    }
    
    /// Determine how many times a block should be repeated
    fn get_block_repeat_count(&self, reader: &mut Cursor<&[u8]>, block_template: &BlockDefinition) -> NetworkResult<u32> {
        match block_template.cardinality {
            Cardinality::Single => Ok(1),
            Cardinality::Multiple => {
                // Multiple blocks have a fixed count (usually specified in template)
                // For now, assume count of 1 unless we can determine otherwise
                Ok(1)
            }
            Cardinality::Variable => {
                // Variable blocks have a count prefix
                if reader.position() >= reader.get_ref().len() as u64 {
                    return Ok(0); // No data left for count
                }
                let count = reader.read_u8().map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read variable block count: {}", e),
                })?;
                Ok(count as u32)
            }
        }
    }
    
    /// Parse a single block according to its template
    fn parse_block(&self, reader: &mut Cursor<&[u8]>, block_template: &BlockDefinition) -> NetworkResult<ParsedBlock> {
        let mut block = ParsedBlock::new(block_template.name.clone());
        
        for field_template in &block_template.fields {
            // Check if we have data before parsing each field
            if reader.position() >= reader.get_ref().len() as u64 {
                debug!("Data ended before field '{}' in block '{}', stopping block parse", 
                       field_template.name, block_template.name);
                break;
            }
            
            match self.parse_field(reader, field_template) {
                Ok(value) => {
                    block.add_field(field_template.name.clone(), value);
                }
                Err(e) => {
                    // Don't fail the whole packet for a single field error
                    debug!("Failed to parse field '{}' in block '{}': {}", 
                           field_template.name, block_template.name, e);
                    break;
                }
            }
        }
        
        Ok(block)
    }
    
    /// Parse a single field according to its type
    fn parse_field(&self, reader: &mut Cursor<&[u8]>, field_template: &FieldDefinition) -> NetworkResult<FieldValue> {
        let field_type = field_template.type_name.trim();
        
        match field_type {
            "U8" => Ok(FieldValue::U8(reader.read_u8().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read U8: {}", e),
            })?)),
            
            "U16" => Ok(FieldValue::U16(reader.read_u16::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read U16: {}", e),
            })?)),
            
            "U32" => Ok(FieldValue::U32(reader.read_u32::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read U32: {}", e),
            })?)),
            
            "U64" => Ok(FieldValue::U64(reader.read_u64::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read U64: {}", e),
            })?)),
            
            "S16" => Ok(FieldValue::S16(reader.read_i16::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read S16: {}", e),
            })?)),
            
            "F32" => Ok(FieldValue::F32(reader.read_f32::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read F32: {}", e),
            })?)),
            
            "F64" => Ok(FieldValue::F64(reader.read_f64::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                reason: format!("Failed to read F64: {}", e),
            })?)),
            
            "LLUUID" => {
                let mut uuid_bytes = [0u8; 16];
                reader.read_exact(&mut uuid_bytes).map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read LLUUID: {}", e),
                })?;
                Ok(FieldValue::LLUUID(uuid_bytes))
            }
            
            "BOOL" => {
                let byte = reader.read_u8().map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read BOOL: {}", e),
                })?;
                Ok(FieldValue::BOOL(byte != 0))
            }
            
            "LLQuaternion" => {
                let mut quat = [0f32; 4];
                for i in 0..4 {
                    quat[i] = reader.read_f32::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                        reason: format!("Failed to read LLQuaternion component {}: {}", i, e),
                    })?;
                }
                Ok(FieldValue::LLQuaternion(quat))
            }
            
            "LLVector3" => {
                let mut vec = [0f32; 3];
                for i in 0..3 {
                    vec[i] = reader.read_f32::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                        reason: format!("Failed to read LLVector3 component {}: {}", i, e),
                    })?;
                }
                Ok(FieldValue::LLVector3(vec))
            }
            
            "Variable 1" => {
                let length = reader.read_u8().map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read Variable1 length: {}", e),
                })? as usize;
                
                let mut data = vec![0u8; length];
                reader.read_exact(&mut data).map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read Variable1 data of length {}: {}", length, e),
                })?;
                Ok(FieldValue::Variable(data))
            }
            
            "Variable 2" => {
                let length = reader.read_u16::<LittleEndian>().map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read Variable2 length: {}", e),
                })? as usize;
                
                let mut data = vec![0u8; length];
                reader.read_exact(&mut data).map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read Variable2 data of length {}: {}", length, e),
                })?;
                Ok(FieldValue::Variable(data))
            }
            
            fixed if fixed.starts_with("Fixed ") => {
                let length_str = fixed.trim_start_matches("Fixed ");
                let length: usize = length_str.parse().map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Invalid fixed length '{}': {}", length_str, e),
                })?;
                
                let mut data = vec![0u8; length];
                reader.read_exact(&mut data).map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read Fixed{} data: {}", length, e),
                })?;
                Ok(FieldValue::Fixed(data))
            }
            
            _ => {
                // For unknown types, try to skip gracefully
                warn!("Unknown field type '{}', attempting to read as U8", field_type);
                Ok(FieldValue::U8(reader.read_u8().map_err(|e| NetworkError::PacketDecode {
                    reason: format!("Failed to read unknown type as U8: {}", e),
                })?))
            }
        }
    }
}

impl Default for TemplateDeserializer {
    fn default() -> Self {
        Self::new()
    }
}