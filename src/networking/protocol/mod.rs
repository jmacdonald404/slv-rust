// Generated modules from build.rs
pub mod generated_messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub mod generated_codecs {
    use super::generated_messages::*;
    include!(concat!(env!("OUT_DIR"), "/codecs.rs"));
}

// Legacy modules (to be replaced by generated code)
pub mod messages;
pub mod codecs;
pub mod region_handshake;
pub mod template_parser;

// Re-export the generated types with different names to avoid conflicts
pub use generated_messages::Message as GeneratedMessage;
pub use generated_codecs::PacketHeader as GeneratedPacketHeader;
pub use generated_codecs::MessageCodec as GeneratedMessageCodec;
