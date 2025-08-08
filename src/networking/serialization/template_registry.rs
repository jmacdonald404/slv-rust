//! Message template registry for runtime packet parsing

use crate::utils::build_utils::template_parser::{MessageDefinition, MessageTemplate, Frequency};
use crate::networking::packets::PacketFrequency;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Global registry of message templates for packet parsing
pub struct TemplateRegistry {
    templates_by_id: HashMap<u32, MessageDefinition>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            templates_by_id: HashMap::new(),
        }
    }
    
    /// Register a message template
    pub fn register(&mut self, template: MessageDefinition) {
        let key = Self::make_key(template.id, &template.frequency);
        self.templates_by_id.insert(key, template);
    }
    
    /// Get a message template by packet ID and frequency
    pub fn get_template(&self, packet_id: u16, frequency: PacketFrequency) -> Option<&MessageDefinition> {
        let template_frequency = match frequency {
            PacketFrequency::High => Frequency::High,
            PacketFrequency::Medium => Frequency::Medium, 
            PacketFrequency::Low => Frequency::Low,
            PacketFrequency::Fixed => Frequency::Fixed,
        };
        
        let key = Self::make_key(packet_id as u32, &template_frequency);
        self.templates_by_id.get(&key)
    }
    
    /// Create a lookup key from packet ID and frequency
    fn make_key(packet_id: u32, frequency: &Frequency) -> u32 {
        match frequency {
            Frequency::High => packet_id,
            Frequency::Medium => (1 << 16) | packet_id,
            Frequency::Low => (2 << 16) | packet_id,
            Frequency::Fixed => (3 << 16) | packet_id,
        }
    }
    
    /// Initialize registry with known templates
    pub fn init_default() -> Self {
        let mut registry = Self::new();
        
        // Parse the message template file at build time and register templates
        if let Ok(template_content) = std::fs::read_to_string("external/master-message-template/message_template.msg") {
            if let Ok(parsed) = crate::utils::build_utils::template_parser::parse(&template_content) {
                for message in parsed.messages {
                    registry.register(message);
                }
            }
        }
        
        registry
    }
}

/// Global template registry singleton
static TEMPLATE_REGISTRY: OnceLock<TemplateRegistry> = OnceLock::new();

/// Get the global template registry
pub fn get_template_registry() -> &'static TemplateRegistry {
    TEMPLATE_REGISTRY.get_or_init(|| TemplateRegistry::init_default())
}

/// Get a message template by packet ID and frequency
pub fn get_message_template(packet_id: u16, frequency: PacketFrequency) -> Option<&'static MessageDefinition> {
    get_template_registry().get_template(packet_id, frequency)
}