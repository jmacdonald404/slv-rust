// Generated modules from build.rs
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub mod codecs {
    use super::messages::*;
    include!(concat!(env!("OUT_DIR"), "/codecs.rs"));
}

pub mod region_handshake;
pub mod sl_compatibility;
pub use crate::utils::build_utils::template_parser;

// Re-export compatibility types for easier use
pub use sl_compatibility::{HandshakeMessage, SLMessageCodec};

// Re-export generated types
pub use messages::Message;
pub use codecs::{MessageCodec, PacketHeader, Encode, Decode};
