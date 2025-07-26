use uuid::Uuid;

/// Commands that can be sent from the application to the networking layer
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// Send a chat message to the simulator
    SendChat {
        message: String,
        channel: i32,
        chat_type: u8,
    },
    
    /// Send an agent update (position, camera, controls)
    SendAgentUpdate {
        position: (f32, f32, f32),
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        controls: u32,
    },
    
    /// Request object information by ID
    RequestObject {
        id: u32,
    },
    
    /// Request texture by UUID
    RequestTexture {
        texture_id: Uuid,
    },
    
    /// Send throttle settings
    SendThrottle {
        throttle: [f32; 7],
    },
    
    /// Initiate logout sequence
    Logout,
    
    /// Send a generic message (for testing or special cases)
    SendRawMessage {
        message: crate::networking::protocol::messages::Message,
    },
}

impl NetworkCommand {
    /// Create a chat command for local channel
    pub fn local_chat(message: String) -> Self {
        Self::SendChat {
            message,
            channel: 0, // Local channel
            chat_type: 1, // Chat type normal
        }
    }
    
    /// Create an agent update command from current state
    pub fn agent_update(
        position: (f32, f32, f32),
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        controls: u32,
    ) -> Self {
        Self::SendAgentUpdate {
            position,
            camera_at,
            camera_eye,
            controls,
        }
    }
}