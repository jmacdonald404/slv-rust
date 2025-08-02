use uuid::Uuid;
use std::time::SystemTime;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};

/// Events sent from the networking layer to the application
/// These are clean, application-friendly data structures

/// Chat message received from the simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatEvent {
    pub sender_name: String,
    pub sender_id: Uuid,
    pub message: String,
    pub channel: i32,
    pub chat_type: u8,
    pub timestamp: SystemTime,
}

/// Object update received from the simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMovementCompleteEvent {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: SystemTime,
}

/// Health/status update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthUpdateEvent {
    pub health: f32,
    pub timestamp: SystemTime,
}

/// Avatar data update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarDataUpdateEvent {
    pub agent_id: Uuid,
    pub firstname: String,
    pub lastname: String,
    pub group_title: String,
    pub timestamp: SystemTime,
}

/// Region handshake completion event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionHandshakeEvent {
    pub region_name: String,
    pub region_id: Uuid,
    pub region_flags: u32,
    pub water_height: f32,
    pub sim_access: u8,
    pub timestamp: SystemTime,
}

/// Keep alive event (for connection monitoring)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAliveEvent {
    pub timestamp: SystemTime,
}

/// Connection status change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatusEvent {
    pub status: ConnectionStatus,
    pub timestamp: SystemTime,
}

/// Connection status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connecting,
    Handshaking,
    Connected,
    Disconnecting,
    Disconnected,
    Error(String),
}

/// Performance metrics update
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Maximum number of events that can be buffered in the event bus
const EVENT_BUS_CAPACITY: usize = 10000;

/// Unified world event enum that encompasses all possible events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldEvent {
    // Legacy events (preserved for compatibility)
    Chat(ChatEvent),
    ObjectUpdate(ObjectUpdateEvent),
    AgentMovementComplete(AgentMovementCompleteEvent),
    HealthUpdate(HealthUpdateEvent),
    AvatarDataUpdate(AvatarDataUpdateEvent),
    RegionHandshake(RegionHandshakeEvent),
    KeepAlive(KeepAliveEvent),
    ConnectionStatus(ConnectionStatusEvent),
    PerformanceMetrics(PerformanceMetricsEvent),
    
    // Extended events for comprehensive coverage
    ObjectKilled {
        region_handle: u64,
        object_id: u32,
    },
    TerrainUpdated {
        region_handle: u64,
        layer_type: u8,
        data: Vec<u8>,
    },
    AgentMovementUpdated {
        agent_id: Uuid,
        position: [f32; 3],
        rotation: [f32; 4], // Quaternion
        velocity: [f32; 3],
    },
    AvatarAppearanceUpdated {
        agent_id: Uuid,
        appearance_data: Vec<u8>,
    },
    InstantMessageReceived {
        sender_id: Uuid,
        sender_name: String,
        message: String,
        im_type: u8,
        session_id: Uuid,
    },
    InventoryUpdated {
        folder_id: Uuid,
        items: Vec<InventoryItemInfo>,
        folders: Vec<InventoryFolderInfo>,
    },
    AssetDownloaded {
        asset_id: Uuid,
        asset_type: u8,
        data: Vec<u8>,
    },
    TextureDownloaded {
        texture_id: Uuid,
        data: Vec<u8>,
    },
    RegionChanged {
        old_region: Option<u64>,
        new_region: u64,
        position: [f32; 3],
    },
    TeleportStarted {
        destination: String,
        teleport_flags: u32,
    },
    TeleportCompleted {
        region_handle: u64,
        position: [f32; 3],
    },
    MoneyBalanceUpdated {
        balance: i32,
    },
    GroupInviteReceived {
        group_id: Uuid,
        group_name: String,
        inviter_id: Uuid,
        inviter_name: String,
    },
    ErrorOccurred {
        error_type: String,
        message: String,
        severity: ErrorSeverity,
    },
}

/// Error severity levels for system events
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Inventory item information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemInfo {
    pub item_id: Uuid,
    pub parent_id: Uuid,
    pub asset_id: Uuid,
    pub name: String,
    pub description: String,
    pub asset_type: i32,
    pub inv_type: i32,
}

/// Inventory folder information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryFolderInfo {
    pub folder_id: Uuid,
    pub parent_id: Uuid,
    pub name: String,
    pub type_default: i32,
    pub version: i32,
}

/// Event bus for world events
#[derive(Debug, Clone)]
pub struct WorldEventBus {
    /// Broadcast sender for world events
    sender: broadcast::Sender<WorldEvent>,
    /// Event statistics
    stats: Arc<RwLock<EventBusStats>>,
}

/// Event bus statistics
#[derive(Debug, Default, Clone)]
pub struct EventBusStats {
    pub total_events_sent: u64,
    pub total_events_received: u64,
    pub events_by_type: HashMap<String, u64>,
    pub active_subscribers: usize,
    pub dropped_events: u64,
}

impl WorldEventBus {
    /// Create a new world event bus
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BUS_CAPACITY);
        
        Self {
            sender,
            stats: Arc::new(RwLock::new(EventBusStats::default())),
        }
    }
    
    /// Emit a world event
    pub async fn emit(&self, event: WorldEvent) {
        let event_type = Self::event_type_name(&event);
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events_sent += 1;
            *stats.events_by_type.entry(event_type.clone()).or_insert(0) += 1;
        }
        
        // Send the event
        match self.sender.send(event.clone()) {
            Ok(subscriber_count) => {
                debug!("📨 Emitted {} event to {} subscribers", event_type, subscriber_count);
            },
            Err(_) => {
                warn!("📨 No subscribers for {} event", event_type);
                
                // Update dropped events counter
                let mut stats = self.stats.write().await;
                stats.dropped_events += 1;
            }
        }
    }
    
    /// Subscribe to world events
    pub fn subscribe(&self) -> WorldEventReceiver {
        let receiver = self.sender.subscribe();
        
        // Update subscriber count
        tokio::spawn({
            let stats = Arc::clone(&self.stats);
            async move {
                let mut stats = stats.write().await;
                stats.active_subscribers += 1;
            }
        });
        
        WorldEventReceiver {
            receiver,
            stats: Arc::clone(&self.stats),
        }
    }
    
    /// Get event bus statistics
    pub async fn stats(&self) -> EventBusStats {
        self.stats.read().await.clone()
    }
    
    /// Get the type name for an event
    fn event_type_name(event: &WorldEvent) -> String {
        match event {
            WorldEvent::Chat(_) => "Chat".to_string(),
            WorldEvent::ObjectUpdate(_) => "ObjectUpdate".to_string(),
            WorldEvent::AgentMovementComplete(_) => "AgentMovementComplete".to_string(),
            WorldEvent::HealthUpdate(_) => "HealthUpdate".to_string(),
            WorldEvent::AvatarDataUpdate(_) => "AvatarDataUpdate".to_string(),
            WorldEvent::RegionHandshake(_) => "RegionHandshake".to_string(),
            WorldEvent::KeepAlive(_) => "KeepAlive".to_string(),
            WorldEvent::ConnectionStatus(_) => "ConnectionStatus".to_string(),
            WorldEvent::PerformanceMetrics(_) => "PerformanceMetrics".to_string(),
            WorldEvent::ObjectKilled { .. } => "ObjectKilled".to_string(),
            WorldEvent::TerrainUpdated { .. } => "TerrainUpdated".to_string(),
            WorldEvent::AgentMovementUpdated { .. } => "AgentMovementUpdated".to_string(),
            WorldEvent::AvatarAppearanceUpdated { .. } => "AvatarAppearanceUpdated".to_string(),
            WorldEvent::InstantMessageReceived { .. } => "InstantMessageReceived".to_string(),
            WorldEvent::InventoryUpdated { .. } => "InventoryUpdated".to_string(),
            WorldEvent::AssetDownloaded { .. } => "AssetDownloaded".to_string(),
            WorldEvent::TextureDownloaded { .. } => "TextureDownloaded".to_string(),
            WorldEvent::RegionChanged { .. } => "RegionChanged".to_string(),
            WorldEvent::TeleportStarted { .. } => "TeleportStarted".to_string(),
            WorldEvent::TeleportCompleted { .. } => "TeleportCompleted".to_string(),
            WorldEvent::MoneyBalanceUpdated { .. } => "MoneyBalanceUpdated".to_string(),
            WorldEvent::GroupInviteReceived { .. } => "GroupInviteReceived".to_string(),
            WorldEvent::ErrorOccurred { .. } => "ErrorOccurred".to_string(),
        }
    }
}

impl Default for WorldEventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// World event receiver for subscribing to events
pub struct WorldEventReceiver {
    receiver: broadcast::Receiver<WorldEvent>,
    stats: Arc<RwLock<EventBusStats>>,
}

impl WorldEventReceiver {
    /// Receive the next world event
    pub async fn recv(&mut self) -> Result<WorldEvent, WorldEventError> {
        match self.receiver.recv().await {
            Ok(event) => {
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.total_events_received += 1;
                }
                
                Ok(event)
            },
            Err(broadcast::error::RecvError::Closed) => {
                Err(WorldEventError::BusClosed)
            },
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("📨 Event receiver lagged, skipped {} events", skipped);
                
                // Update statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.dropped_events += skipped;
                }
                
                Err(WorldEventError::Lagged { skipped })
            }
        }
    }
}

impl Drop for WorldEventReceiver {
    fn drop(&mut self) {
        // Update subscriber count
        let stats = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut stats = stats.write().await;
            if stats.active_subscribers > 0 {
                stats.active_subscribers -= 1;
            }
        });
    }
}

/// Errors that can occur when receiving world events
#[derive(Debug, thiserror::Error)]
pub enum WorldEventError {
    #[error("Event bus has been closed")]
    BusClosed,
    
    #[error("Receiver lagged behind, skipped {skipped} events")]
    Lagged { skipped: u64 },
}

/// Trait for components that can handle world events
pub trait WorldEventHandler: Send + Sync {
    /// Handle a world event
    fn handle_event(&mut self, event: &WorldEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Get the name of this event handler
    fn name(&self) -> &'static str;
    
    /// Check if this handler is interested in a specific event type
    fn is_interested_in(&self, event: &WorldEvent) -> bool;
}