//! Second Life viewer effects system
//! 
//! This module implements the ViewerEffect message system which is used for
//! visual effects like pointing gestures, beams, spheres, and other particle effects.

use crate::networking::packets::generated::{ViewerEffect, EffectBlock};
use crate::networking::packets::types::{LLUUID, LLVariable1};
use uuid::Uuid;
use std::collections::HashMap;
use tracing::{debug, info};

/// Types of viewer effects supported by Second Life
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectType {
    /// Text effect (floating text)
    Text = 0,
    /// Icon effect  
    Icon = 1,
    /// Connector effect (line between objects)
    Connector = 2,
    /// Flexible object effect
    Flexible = 3,
    /// Beam effect (pointing gesture)
    Beam = 4,
    /// Glow effect
    Glow = 5,
    /// Point at effect (Type=9 from hippolog)
    PointAt = 9,
    /// Look at effect
    LookAt = 10,
    /// Edit beam effect
    EditBeam = 11,
    /// Sphere effect
    Sphere = 13,
}

impl From<EffectType> for u8 {
    fn from(effect_type: EffectType) -> u8 {
        effect_type as u8
    }
}

/// Color representation for effects
#[derive(Debug, Clone)]
pub struct EffectColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl EffectColor {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }

    pub fn red() -> Self {
        Self::new(255, 0, 0, 255)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        vec![self.r, self.g, self.b, self.a]
    }
}

/// Position in 3D space
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn origin() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// TypeData for different effect types - based on hippolog analysis
#[derive(Debug, Clone)]
pub enum EffectTypeData {
    /// PointAt effect data (Type=9) - contains source and target positions
    PointAt {
        source_pos: Position,
        target_pos: Position,
        target_id: Option<Uuid>,
    },
    /// Beam effect data (Type=4)
    Beam {
        source_pos: Position,
        target_pos: Position,
    },
    /// Sphere effect data (Type=13)
    Sphere {
        center: Position,
        radius: f32,
    },
    /// Generic effect with raw data
    Raw(Vec<u8>),
}

impl EffectTypeData {
    /// Convert TypeData to bytes format expected by Second Life
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            EffectTypeData::PointAt { source_pos, target_pos, target_id } => {
                let mut data = vec![0u8; 32]; // Start with 32 zero bytes as seen in hippolog
                
                // Add source position (3 floats = 12 bytes)
                data.extend_from_slice(&source_pos.x.to_le_bytes());
                data.extend_from_slice(&source_pos.y.to_le_bytes());
                data.extend_from_slice(&source_pos.z.to_le_bytes());
                
                // Add padding
                data.extend_from_slice(&[0u8; 4]);
                
                // Add target position (3 floats = 12 bytes)
                data.extend_from_slice(&target_pos.x.to_le_bytes());
                data.extend_from_slice(&target_pos.y.to_le_bytes());
                data.extend_from_slice(&target_pos.z.to_le_bytes());
                
                // Add final padding
                data.extend_from_slice(&[0u8; 1]);
                
                data
            },
            EffectTypeData::Beam { source_pos, target_pos } => {
                let mut data = Vec::new();
                data.extend_from_slice(&source_pos.x.to_le_bytes());
                data.extend_from_slice(&source_pos.y.to_le_bytes());
                data.extend_from_slice(&source_pos.z.to_le_bytes());
                data.extend_from_slice(&target_pos.x.to_le_bytes());
                data.extend_from_slice(&target_pos.y.to_le_bytes());
                data.extend_from_slice(&target_pos.z.to_le_bytes());
                data
            },
            EffectTypeData::Sphere { center, radius } => {
                let mut data = Vec::new();
                data.extend_from_slice(&center.x.to_le_bytes());
                data.extend_from_slice(&center.y.to_le_bytes());
                data.extend_from_slice(&center.z.to_le_bytes());
                data.extend_from_slice(&radius.to_le_bytes());
                data
            },
            EffectTypeData::Raw(bytes) => bytes.clone(),
        }
    }
}

/// Configuration for creating a viewer effect
#[derive(Debug, Clone)]
pub struct EffectConfig {
    pub effect_type: EffectType,
    pub duration: f32,
    pub color: EffectColor,
    pub type_data: EffectTypeData,
    pub agent_id: Uuid,
}

impl EffectConfig {
    /// Create a point-at effect configuration
    pub fn point_at(agent_id: Uuid, source_pos: Position, target_pos: Position) -> Self {
        Self {
            effect_type: EffectType::PointAt,
            duration: 0.5, // Default 0.5 seconds as seen in hippolog
            color: EffectColor::white(),
            type_data: EffectTypeData::PointAt {
                source_pos,
                target_pos,
                target_id: None,
            },
            agent_id,
        }
    }

    /// Create a beam effect configuration
    pub fn beam(agent_id: Uuid, source_pos: Position, target_pos: Position) -> Self {
        Self {
            effect_type: EffectType::Beam,
            duration: 1.0,
            color: EffectColor::red(),
            type_data: EffectTypeData::Beam { source_pos, target_pos },
            agent_id,
        }
    }
}

/// ViewerEffect message builder and manager
#[derive(Debug)]
pub struct EffectManager {
    /// Active effects by effect ID
    active_effects: HashMap<Uuid, EffectConfig>,
    /// Next effect ID counter
    next_effect_id: u32,
}

impl EffectManager {
    pub fn new() -> Self {
        Self {
            active_effects: HashMap::new(),
            next_effect_id: 1,
        }
    }

    /// Generate a unique effect ID
    fn generate_effect_id(&mut self) -> Uuid {
        let id = Uuid::new_v4();
        self.next_effect_id += 1;
        id
    }

    /// Create a ViewerEffect message from an effect configuration
    pub fn create_viewer_effect(&mut self, session_id: Uuid, config: EffectConfig) -> ViewerEffect {
        let effect_id = self.generate_effect_id();
        
        info!("🎭 Creating ViewerEffect:");
        info!("  Type: {:?} ({})", config.effect_type, u8::from(config.effect_type));
        info!("  Duration: {} seconds", config.duration);
        info!("  Color: {:?}", config.color);
        info!("  Agent: {}", config.agent_id);
        info!("  Effect ID: {}", effect_id);
        
        // Convert type data to bytes
        let type_data_bytes = config.type_data.to_bytes();
        debug!("  TypeData: {} bytes = {:02x?}", type_data_bytes.len(), type_data_bytes);
        
        // Create the effect block
        let effect_block = EffectBlock {
            id: LLUUID::from(effect_id),
            agent_id: LLUUID::from(config.agent_id),
            r#type: u8::from(config.effect_type),
            duration: config.duration,
            color: config.color.to_bytes(),
            type_data: LLVariable1::new(type_data_bytes),
        };

        // Create the ViewerEffect message
        let viewer_effect = ViewerEffect {
            agent_id: LLUUID::from(config.agent_id),
            session_id: LLUUID::from(session_id),
            effect: vec![effect_block],
        };

        // Store the effect
        self.active_effects.insert(effect_id, config);

        viewer_effect
    }

    /// Create a point-at effect (Type=9 as seen in hippolog)
    pub fn create_point_at_effect(
        &mut self,
        agent_id: Uuid,
        session_id: Uuid,
        source_pos: Position,
        target_pos: Position,
    ) -> ViewerEffect {
        let config = EffectConfig::point_at(agent_id, source_pos, target_pos);
        self.create_viewer_effect(session_id, config)
    }

    /// Create a beam effect
    pub fn create_beam_effect(
        &mut self,
        agent_id: Uuid,
        session_id: Uuid,
        source_pos: Position,
        target_pos: Position,
    ) -> ViewerEffect {
        let config = EffectConfig::beam(agent_id, source_pos, target_pos);
        self.create_viewer_effect(session_id, config)
    }

    /// Get count of active effects
    pub fn active_effect_count(&self) -> usize {
        self.active_effects.len()
    }

    /// Clear expired effects (in a real implementation, this would check timestamps)
    pub fn cleanup_expired_effects(&mut self) {
        // For now, just clear all effects - in a real implementation,
        // this would check effect timestamps and durations
        if !self.active_effects.is_empty() {
            debug!("Cleaning up {} active effects", self.active_effects.len());
            self.active_effects.clear();
        }
    }
}

impl Default for EffectManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_type_conversion() {
        assert_eq!(u8::from(EffectType::PointAt), 9);
        assert_eq!(u8::from(EffectType::Beam), 4);
        assert_eq!(u8::from(EffectType::Sphere), 13);
    }

    #[test]
    fn test_effect_color() {
        let white = EffectColor::white();
        assert_eq!(white.to_bytes(), vec![255, 255, 255, 255]);
        
        let red = EffectColor::red();
        assert_eq!(red.to_bytes(), vec![255, 0, 0, 255]);
    }

    #[test]
    fn test_point_at_type_data() {
        let source = Position::new(100.0, 200.0, 300.0);
        let target = Position::new(150.0, 250.0, 350.0);
        
        let type_data = EffectTypeData::PointAt {
            source_pos: source,
            target_pos: target,
            target_id: None,
        };
        
        let bytes = type_data.to_bytes();
        assert!(bytes.len() > 48); // Should have 32 zeros + positions + padding
        
        // Check that we have the expected structure
        assert!(bytes.starts_with(&[0u8; 32])); // First 32 bytes should be zeros
    }

    #[test]
    fn test_effect_manager() {
        let mut manager = EffectManager::new();
        let agent_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        
        let source = Position::new(0.0, 0.0, 0.0);
        let target = Position::new(10.0, 10.0, 10.0);
        
        let effect = manager.create_point_at_effect(agent_id, session_id, source, target);
        
        assert_eq!(effect.agent_id, LLUUID::from(agent_id));
        assert_eq!(effect.session_id, LLUUID::from(session_id));
        assert_eq!(effect.effect.len(), 1);
        assert_eq!(effect.effect[0].r#type, 9); // PointAt type
        assert_eq!(effect.effect[0].duration, 0.5);
    }
}