use uuid::Uuid;
use std::time::SystemTime;

/// Events sent from the networking layer to the application
/// These are clean, application-friendly data structures

/// Chat message received from the simulator
#[derive(Debug, Clone)]
pub struct ChatEvent {
    pub sender_name: String,
    pub sender_id: Uuid,
    pub message: String,
    pub channel: i32,
    pub chat_type: u8,
    pub timestamp: SystemTime,
}

/// Object update received from the simulator
#[derive(Debug, Clone)]
pub struct ObjectUpdateEvent {
    pub object_id: u32,
    pub position: (f32, f32, f32),
    pub rotation: (f32, f32, f32, f32), // quaternion (x, y, z, w)
    pub velocity: (f32, f32, f32),
    pub angular_velocity: (f32, f32, f32),
    pub scale: (f32, f32, f32),
    pub timestamp: SystemTime,
}

/// Agent movement completion event
#[derive(Debug, Clone)]
pub struct AgentMovementCompleteEvent {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: SystemTime,
}

/// Health/status update event
#[derive(Debug, Clone)]
pub struct HealthUpdateEvent {
    pub health: f32,
    pub timestamp: SystemTime,
}

/// Avatar data update event
#[derive(Debug, Clone)]
pub struct AvatarDataUpdateEvent {
    pub agent_id: Uuid,
    pub firstname: String,
    pub lastname: String,
    pub group_title: String,
    pub timestamp: SystemTime,
}

/// Region handshake completion event
#[derive(Debug, Clone)]
pub struct RegionHandshakeEvent {
    pub region_name: String,
    pub region_id: Uuid,
    pub region_flags: u32,
    pub water_height: f32,
    pub sim_access: u8,
    pub timestamp: SystemTime,
}

/// Keep alive event (for connection monitoring)
#[derive(Debug, Clone)]
pub struct KeepAliveEvent {
    pub timestamp: SystemTime,
}

/// Connection status change event
#[derive(Debug, Clone)]
pub struct ConnectionStatusEvent {
    pub status: ConnectionStatus,
    pub timestamp: SystemTime,
}

/// Connection status enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Handshaking,
    Connected,
    Disconnecting,
    Disconnected,
    Error(String),
}

/// Performance metrics update
#[derive(Debug, Clone)]
pub struct PerformanceMetricsEvent {
    pub avg_fps: f32,
    pub frame_time_ms: f32,
    pub network_latency_ms: u32,
    pub packet_loss_percent: f32,
    pub timestamp: SystemTime,
}

impl ChatEvent {
    /// Create a new chat event
    pub fn new(sender_name: String, sender_id: Uuid, message: String, channel: i32, chat_type: u8) -> Self {
        Self {
            sender_name,
            sender_id,
            message,
            channel,
            chat_type,
            timestamp: SystemTime::now(),
        }
    }
    
    /// Check if this is a local chat message
    pub fn is_local_chat(&self) -> bool {
        self.channel == 0
    }
    
    /// Check if this is a private message
    pub fn is_private_message(&self) -> bool {
        self.chat_type == 3 // IM chat type
    }
}

impl ObjectUpdateEvent {
    /// Create a new object update event
    pub fn new(object_id: u32, position: (f32, f32, f32)) -> Self {
        Self {
            object_id,
            position,
            rotation: (0.0, 0.0, 0.0, 1.0), // Identity quaternion
            velocity: (0.0, 0.0, 0.0),
            angular_velocity: (0.0, 0.0, 0.0),
            scale: (1.0, 1.0, 1.0),
            timestamp: SystemTime::now(),
        }
    }
}

impl ConnectionStatusEvent {
    /// Create a connection status event
    pub fn new(status: ConnectionStatus) -> Self {
        Self {
            status,
            timestamp: SystemTime::now(),
        }
    }
    
    /// Create an error status event
    pub fn error(message: String) -> Self {
        Self::new(ConnectionStatus::Error(message))
    }
}