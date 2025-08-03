//! Second Life protocol data types
//! 
//! These types provide exact binary compatibility with the official Second Life viewer
//! while leveraging Rust's type system for safety and performance.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 32-bit unsigned integer (little-endian)
pub type U32 = u32;

/// 16-bit unsigned integer (little-endian) 
pub type U16 = u16;

/// 8-bit unsigned integer
pub type U8 = u8;

/// 8-bit signed integer
pub type S8 = i8;

/// 16-bit signed integer (little-endian)
pub type S16 = i16;

/// 32-bit signed integer (little-endian)
pub type S32 = i32;

/// 64-bit signed integer (little-endian)
pub type S64 = i64;

/// 32-bit IEEE 754 floating point (little-endian)
pub type F32 = f32;

/// 64-bit IEEE 754 floating point (little-endian)
pub type F64 = f64;

/// UUID (16 bytes, RFC 4122 format)
pub type LLUUID = Uuid;

/// Boolean value (1 byte: 0x00 = false, 0x01 = true)
pub type BOOL = bool;

/// IP Address (4 bytes, network byte order)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IPADDR(pub u32);

impl IPADDR {
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        IPADDR(((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32))
    }
    
    pub fn to_std_addr(&self) -> std::net::Ipv4Addr {
        std::net::Ipv4Addr::from(self.0.to_be())
    }
}

/// IP Port (2 bytes, network byte order)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IPPORT(pub u16);

impl IPPORT {
    pub fn new(port: u16) -> Self {
        IPPORT(port.to_be())
    }
    
    pub fn to_host_order(&self) -> u16 {
        u16::from_be(self.0)
    }
}

/// 3D Vector (12 bytes: 3 x F32)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LLVector3 {
    pub x: F32,
    pub y: F32, 
    pub z: F32,
}

impl LLVector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// 3D Vector with double precision (24 bytes: 3 x F64)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LLVector3d {
    pub x: F64,
    pub y: F64,
    pub z: F64,
}

impl LLVector3d {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// 4D Vector (16 bytes: 4 x F32)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LLVector4 {
    pub x: F32,
    pub y: F32,
    pub z: F32,
    pub w: F32,
}

impl LLVector4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

/// Quaternion rotation (12 bytes: 3 x F32)
/// Protocol optimization: W component calculated as sqrt(1 - (X² + Y² + Z²))
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LLQuaternion {
    pub x: F32,
    pub y: F32,
    pub z: F32,
}

impl LLQuaternion {
    /// Create quaternion from X, Y, Z components (W calculated automatically)
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    
    /// Create quaternion from full X, Y, Z, W components (W ignored for protocol)
    pub fn from_xyzw(x: f32, y: f32, z: f32, _w: f32) -> Self {
        Self { x, y, z }
    }
    
    /// Identity quaternion
    pub fn identity() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
    
    /// Calculate W component (for local use)
    pub fn calculate_w(&self) -> f32 {
        let sum_sq = self.x * self.x + self.y * self.y + self.z * self.z;
        if sum_sq < 1.0 {
            (1.0 - sum_sq).sqrt()
        } else {
            0.0
        }
    }
    
    /// Get full quaternion with calculated W
    pub fn to_full_quat(&self) -> (f32, f32, f32, f32) {
        (self.x, self.y, self.z, self.calculate_w())
    }
}

/// Variable-length string (1 byte length + data)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LLVariable1 {
    pub data: Vec<u8>,
}

impl LLVariable1 {
    pub fn new(data: Vec<u8>) -> Self {
        assert!(data.len() <= 255, "LLVariable1 data too long");
        Self { data }
    }
    
    pub fn from_string(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
    }
    
    pub fn to_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.data.clone())
    }
}

/// Variable-length string (2 byte big-endian length + data)
/// Protocol compliance: Length prefix uses big-endian encoding
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LLVariable2 {
    pub data: Vec<u8>,
}

impl LLVariable2 {
    pub fn new(data: Vec<u8>) -> Self {
        assert!(data.len() <= 65535, "LLVariable2 data too long");
        Self { data }
    }
    
    pub fn from_string(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
    }
    
    pub fn to_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.data.clone())
    }
    
    /// Serialize with big-endian length prefix (protocol compliance)
    pub fn serialize_protocol(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(2 + self.data.len());
        result.extend_from_slice(&(self.data.len() as u16).to_be_bytes()); // Big-endian
        result.extend_from_slice(&self.data);
        result
    }
    
    /// Deserialize from protocol format with big-endian length prefix
    pub fn deserialize_protocol(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 {
            return None;
        }
        
        let length = u16::from_be_bytes([data[0], data[1]]) as usize; // Big-endian
        if data.len() < 2 + length {
            return None;
        }
        
        let var_data = data[2..2 + length].to_vec();
        Some((LLVariable2::new(var_data), 2 + length))
    }
}

/// Fixed-length string (256 bytes) - simplified for serde compatibility
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LLFixed256 {
    pub data: Vec<u8>,
}

impl LLFixed256 {
    pub fn new(data: Vec<u8>) -> Self {
        let mut padded = data;
        padded.resize(256, 0);
        Self { data: padded }
    }
    
    pub fn from_string(s: &str) -> Self {
        let mut data = s.as_bytes().to_vec();
        data.resize(256, 0);
        Self { data }
    }
    
    pub fn to_string(&self) -> Result<String, std::string::FromUtf8Error> {
        // Find the first null byte or use the entire array
        let end = self.data.iter().position(|&b| b == 0).unwrap_or(self.data.len());
        String::from_utf8(self.data[..end].to_vec())
    }
}

/// 64-bit unsigned integer (little-endian)
pub type U64 = u64;

/// Color value (4 bytes: R, G, B, A)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LLColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl LLColor {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    
    pub fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }
    
    pub fn black() -> Self {
        Self::new(0, 0, 0, 255)
    }
}

/// 16-bit unsigned 3D vector (6 bytes: 3 x U16)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct U16Vec3 {
    pub x: U16,
    pub y: U16,
    pub z: U16,
}

impl U16Vec3 {
    pub fn new(x: u16, y: u16, z: u16) -> Self {
        Self { x, y, z }
    }
    
    pub fn zero() -> Self {
        Self::new(0, 0, 0)
    }
}

/// 16-bit unsigned quaternion (8 bytes: 4 x U16)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct U16Quat {
    pub x: U16,
    pub y: U16,
    pub z: U16,
    pub w: U16,
}

impl U16Quat {
    pub fn new(x: u16, y: u16, z: u16, w: u16) -> Self {
        Self { x, y, z, w }
    }
    
    pub fn identity() -> Self {
        Self::new(0, 0, 0, 32767) // Scaled for 16-bit
    }
}

/// Array of 16-bit signed integers (variable length)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct S16Array {
    pub data: Vec<S16>,
}

impl S16Array {
    pub fn new(data: Vec<i16>) -> Self {
        Self { data }
    }
    
    pub fn empty() -> Self {
        Self { data: Vec::new() }
    }
}

/// Null/placeholder type for protocol messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MVTNull;

/// End of list marker for protocol messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MVTEol;