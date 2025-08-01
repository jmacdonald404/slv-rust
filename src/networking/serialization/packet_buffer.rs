//! Efficient packet buffer for parsing Second Life protocol data

use bytes::Buf;
use crate::networking::{NetworkError, NetworkResult};

/// Efficient buffer for parsing Second Life packet data
pub struct PacketBuffer<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> PacketBuffer<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
    
    pub fn remaining_bytes(&self) -> &[u8] {
        &self.data[self.position..]
    }
    
    pub fn has_remaining(&self) -> bool {
        self.position < self.data.len()
    }
    
    pub fn position(&self) -> usize {
        self.position
    }
    
    pub fn set_position(&mut self, pos: usize) {
        self.position = std::cmp::min(pos, self.data.len());
    }
    
    pub fn advance(&mut self, n: usize) {
        self.position = std::cmp::min(self.position + n, self.data.len());
    }
    
    pub fn get_u8(&mut self) -> u8 {
        if self.remaining() < 1 {
            return 0; // Return 0 for out-of-bounds reads (matches SL behavior)
        }
        let value = self.data[self.position];
        self.position += 1;
        value
    }
    
    pub fn get_u16(&mut self) -> u16 {
        if self.remaining() < 2 {
            // Partial read - get what we can
            let mut bytes = [0u8; 2];
            for i in 0..std::cmp::min(2, self.remaining()) {
                bytes[i] = self.data[self.position + i];
            }
            self.position += std::cmp::min(2, self.remaining());
            return u16::from_le_bytes(bytes);
        }
        
        let bytes = [self.data[self.position], self.data[self.position + 1]];
        self.position += 2;
        u16::from_le_bytes(bytes)
    }
    
    pub fn get_u32(&mut self) -> u32 {
        if self.remaining() < 4 {
            // Partial read
            let mut bytes = [0u8; 4];
            for i in 0..std::cmp::min(4, self.remaining()) {
                bytes[i] = self.data[self.position + i];
            }
            self.position += std::cmp::min(4, self.remaining());
            return u32::from_be_bytes(bytes); // Sequence numbers are big-endian
        }
        
        let bytes = [
            self.data[self.position],
            self.data[self.position + 1], 
            self.data[self.position + 2],
            self.data[self.position + 3],
        ];
        self.position += 4;
        u32::from_be_bytes(bytes) // Sequence numbers are big-endian
    }
    
    pub fn get_u64(&mut self) -> u64 {
        if self.remaining() < 8 {
            // Partial read
            let mut bytes = [0u8; 8];
            for i in 0..std::cmp::min(8, self.remaining()) {
                bytes[i] = self.data[self.position + i];
            }
            self.position += std::cmp::min(8, self.remaining());
            return u64::from_le_bytes(bytes);
        }
        
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.position..self.position + 8]);
        self.position += 8;
        u64::from_le_bytes(bytes)
    }
    
    pub fn get_f32(&mut self) -> f32 {
        let bits = self.get_u32_le();
        f32::from_bits(bits)
    }
    
    pub fn get_f64(&mut self) -> f64 {
        let bits = self.get_u64();
        f64::from_bits(bits)
    }
    
    /// Get u32 in little-endian format (for most data fields)
    pub fn get_u32_le(&mut self) -> u32 {
        if self.remaining() < 4 {
            let mut bytes = [0u8; 4];
            for i in 0..std::cmp::min(4, self.remaining()) {
                bytes[i] = self.data[self.position + i];
            }
            self.position += std::cmp::min(4, self.remaining());
            return u32::from_le_bytes(bytes);
        }
        
        let bytes = [
            self.data[self.position],
            self.data[self.position + 1], 
            self.data[self.position + 2],
            self.data[self.position + 3],
        ];
        self.position += 4;
        u32::from_le_bytes(bytes)
    }
    
    pub fn get_bytes(&mut self, len: usize) -> Vec<u8> {
        let actual_len = std::cmp::min(len, self.remaining());
        let mut result = vec![0u8; len];
        
        if actual_len > 0 {
            result[..actual_len].copy_from_slice(&self.data[self.position..self.position + actual_len]);
            self.position += actual_len;
        }
        
        result
    }
    
    pub fn get_bytes_slice(&mut self, len: usize) -> &[u8] {
        let actual_len = std::cmp::min(len, self.remaining());
        let slice = &self.data[self.position..self.position + actual_len];
        self.position += actual_len;
        slice
    }
    
    /// Get a variable-length field with 1-byte length prefix
    pub fn get_variable1(&mut self) -> Vec<u8> {
        let len = self.get_u8() as usize;
        self.get_bytes(len)
    }
    
    /// Get a variable-length field with 2-byte length prefix
    pub fn get_variable2(&mut self) -> Vec<u8> {
        let len = self.get_u16() as usize;
        self.get_bytes(len)
    }
    
    /// Peek at next byte without advancing position
    pub fn peek_u8(&self) -> Option<u8> {
        if self.remaining() > 0 {
            Some(self.data[self.position])
        } else {
            None
        }
    }
    
    /// Check if we have at least n bytes remaining
    pub fn check_remaining(&self, n: usize) -> NetworkResult<()> {
        if self.remaining() < n {
            Err(NetworkError::PacketDecode {
                reason: format!("Not enough data: need {}, have {}", n, self.remaining()),
            })
        } else {
            Ok(())
        }
    }
}