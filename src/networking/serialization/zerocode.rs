//! Second Life zerocoding implementation
//! 
//! Zerocoding compresses sequences of zero bytes in packets to reduce bandwidth.
//! This implementation matches the exact behavior of the official viewer.

use crate::networking::{NetworkError, NetworkResult};

/// Encode data using Second Life's zerocoding algorithm
/// 
/// Zerocoding replaces sequences of zero bytes with a special marker:
/// - 0x00 followed by a count byte indicates (count) zero bytes
/// - 0x00 0x00 represents a literal 0x00 byte
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    
    while i < data.len() {
        if data[i] == 0 {
            // Count consecutive zeros
            let mut zero_count = 0;
            let mut j = i;
            
            while j < data.len() && data[j] == 0 && zero_count < 255 {
                zero_count += 1;
                j += 1;
            }
            
            if zero_count == 1 {
                // Single zero: encode as 0x00 0x00
                result.push(0x00);
                result.push(0x00);
            } else {
                // Multiple zeros: encode as 0x00 count
                result.push(0x00);
                result.push(zero_count as u8);
            }
            
            i = j;
        } else {
            // Non-zero byte: copy as-is
            result.push(data[i]);
            i += 1;
        }
    }
    
    result
}

/// Decode zerocoded data back to original format
pub fn decode(data: &[u8]) -> NetworkResult<Vec<u8>> {
    let mut result = Vec::with_capacity(data.len() * 2); // Estimate capacity
    let mut i = 0;
    
    while i < data.len() {
        if data[i] == 0x00 {
            if i + 1 >= data.len() {
                return Err(NetworkError::PacketDecode {
                    reason: "Truncated zerocode sequence".to_string(),
                });
            }
            
            let count = data[i + 1];
            if count == 0 {
                // 0x00 0x00 represents a literal 0x00
                result.push(0x00);
            } else {
                // 0x00 count represents count zero bytes
                result.extend(std::iter::repeat(0x00).take(count as usize));
            }
            
            i += 2;
        } else {
            // Non-zero byte: copy as-is
            result.push(data[i]);
            i += 1;
        }
    }
    
    Ok(result)
}

/// Check if data would benefit from zerocoding
/// Returns true if encoding would reduce size by at least 10%
pub fn should_encode(data: &[u8]) -> bool {
    if data.len() < 16 {
        return false; // Too small to benefit
    }
    
    let encoded = encode(data);
    let savings = data.len().saturating_sub(encoded.len());
    let savings_percent = (savings * 100) / data.len();
    
    savings_percent >= 10
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_single_zero() {
        let data = vec![1, 0, 2];
        let encoded = encode(&data);
        assert_eq!(encoded, vec![1, 0, 0, 2]);
        
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
    
    #[test]
    fn test_multiple_zeros() {
        let data = vec![1, 0, 0, 0, 2];
        let encoded = encode(&data);
        assert_eq!(encoded, vec![1, 0, 3, 2]);
        
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
    
    #[test]
    fn test_max_zeros() {
        let data = vec![0; 255];
        let encoded = encode(&data);
        assert_eq!(encoded, vec![0, 255]);
        
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
    
    #[test]
    fn test_no_zeros() {
        let data = vec![1, 2, 3, 4, 5];
        let encoded = encode(&data);
        assert_eq!(encoded, data);
        
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
    
    #[test]
    fn test_overflow_zeros() {
        let data = vec![0; 300]; // More than 255 zeros
        let encoded = encode(&data);
        // Should be encoded as two sequences: 255 zeros + 45 zeros
        assert_eq!(encoded, vec![0, 255, 0, 45]);
        
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}