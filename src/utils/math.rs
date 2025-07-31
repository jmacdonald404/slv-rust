use std::fmt;

/// 3D Vector for Second Life coordinates and rotations
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Parse vector from Second Life format: "[r1.0, r0.0, r0.0]" or "r1,0,0"
    pub fn parse_sl_format(value: &str) -> Result<Self, String> {
        let cleaned = value
            .trim_start_matches(['r', '['])
            .trim_end_matches(']')
            .replace('r', "");

        let coords: Result<Vec<f32>, _> = cleaned
            .split(',')
            .map(|s| s.trim().parse())
            .collect();

        match coords {
            Ok(coords) if coords.len() >= 3 => {
                Ok(Self::new(coords[0], coords[1], coords[2]))
            }
            _ => Err(format!("Invalid vector format: {}", value)),
        }
    }

    /// Convert to array for compatibility
    pub fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[r{}, r{}, r{}]", self.x, self.y, self.z)
    }
}

/// 4D Vector for quaternions and other 4D data
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize, Default)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Parse from Second Life format
    pub fn parse_sl_format(value: &str) -> Result<Self, String> {
        let cleaned = value
            .trim_start_matches(['r', '['])
            .trim_end_matches(']')
            .replace('r', "");

        let coords: Result<Vec<f32>, _> = cleaned
            .split(',')
            .map(|s| s.trim().parse())
            .collect();

        match coords {
            Ok(coords) if coords.len() >= 4 => {
                Ok(Self::new(coords[0], coords[1], coords[2], coords[3]))
            }
            _ => Err(format!("Invalid vector4 format: {}", value)),
        }
    }
}

/// Region handle for Second Life grid coordinates
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize, Default)]
pub struct RegionHandle {
    pub x: i32,
    pub y: i32,
}

impl RegionHandle {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Parse region handle from Second Life format: "[r123456, r789012]"
    pub fn parse_sl_format(value: &str) -> Result<Self, String> {
        let cleaned = value
            .trim_start_matches('[')
            .trim_end_matches(']')
            .replace('r', "");

        let coords: Result<Vec<i32>, _> = cleaned
            .split(',')
            .map(|s| s.trim().parse())
            .collect();

        match coords {
            Ok(coords) if coords.len() >= 2 => {
                Ok(Self::new(coords[0], coords[1]))
            }
            _ => Err(format!("Invalid region handle format: {}", value)),
        }
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        format!("[r{}, r{}]", self.x, self.y)
    }
}

/// Utility functions for parsing Second Life data formats
pub mod parsing {
    use super::*;

    /// Parse a boolean value from Second Life format
    pub fn parse_bool(value: &str) -> Result<bool, String> {
        match value.to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            _ => Err(format!("Invalid boolean value: {}", value)),
        }
    }

    /// Parse a UUID from string
    pub fn parse_uuid(value: &str) -> Result<uuid::Uuid, String> {
        uuid::Uuid::parse_str(value)
            .map_err(|e| format!("Invalid UUID: {} - {}", value, e))
    }

    /// Parse a number (integer or float)
    pub fn parse_number(value: &str) -> Result<f64, String> {
        value.parse::<f64>()
            .map_err(|e| format!("Invalid number: {} - {}", value, e))
    }

    /// Parse an array of strings from comma-separated format
    pub fn parse_string_array(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Parse a nested object from JSON-like string
    pub fn parse_nested_object(value: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(value)
            .map_err(|e| format!("Invalid JSON object: {} - {}", value, e))
    }
}
