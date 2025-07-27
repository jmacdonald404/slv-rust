use crate::networking::protocol::{HandshakeMessage, SLMessageCodec, Message, MessageCodec, PacketHeader};
use crate::networking::commands::NetworkCommand;
use crate::world::*;
use crate::config::PerformanceSettingsHandle;
use tracing::{info, warn};
use std::net::SocketAddr;
use std::io;
use std::collections::HashMap;
use tokio::time::{self, Instant, Duration};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::networking::transport::UdpTransport;

/// Configuration for handshake behavior
/// 
/// This configuration allows fine-tuning of the Second Life login handshake process
/// to work around known protocol issues or optimize for different network conditions.
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    /// Delay before sending RegionHandshakeReply (in milliseconds)
    /// 
    /// Set to 0 to disable delay for maximum performance (default).
    /// Set to 2000 for official spec compatibility to reduce interest list errors (BUG-233107).
    /// 
    /// This delay is applied in ALL RegionHandshakeReply sending paths:
    /// - UDP background task (direct from RegionHandshake reception)
    /// - Generated messages handshake flow
    /// - handle_incoming_message method
    /// 
    /// Usage:
    /// - Environment variable: SLV_HANDSHAKE_DELAY_MS=2000
    /// - Runtime: circuit.set_region_handshake_delay(2000).await
    pub region_handshake_reply_delay_ms: u64,
}

impl Default for HandshakeConfig {
    fn default() -> Self {
        Self {
            // Default to 2000ms for official spec compatibility (BUG-233107)
            region_handshake_reply_delay_ms: 2000,
        }
    }
}

// Handshake State Machine for Second Life UDP Login
// Enforces strict ordering: UseCircuitCode -> CompleteAgentMovement -> (wait for RegionHandshake) -> RegionHandshakeReply -> AgentThrottle -> AgentUpdate -> HandshakeComplete
// Each handshake message is sent only once, and only after the previous step is complete. State is tracked per circuit/session.
// All handshake message sending is centralized in advance_handshake().
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum HandshakeState {
    NotStarted,
    SentUseCircuitCode,
    SentCompleteAgentMovement,
    ReceivedRegionHandshake,
    SentRegionHandshakeReply,
    SentAgentThrottle,
    SentFirstAgentUpdate,
    HandshakeComplete,
}

#[derive(Clone)]
pub struct AgentState {
    pub position: (f32, f32, f32),
    pub camera_at: (f32, f32, f32),
    pub camera_eye: (f32, f32, f32),
    pub controls: u32,
}

// Default network settings - can be overridden by performance profile
const DEFAULT_RETRANSMISSION_TIMEOUT_MS: u64 = 200;
const DEFAULT_MAX_RETRANSMISSIONS: u32 = 5;

// TODO: Refactor for proxy support. UdpSocketExt removed.
pub struct Circuit {
    transport: Arc<Mutex<UdpTransport>>,
    next_sequence_number: u32,
    next_expected_sequence_number: Arc<Mutex<u32>>,
    unacked_messages: Arc<Mutex<HashMap<u32, (HandshakeMessage, Instant, u32, SocketAddr, Vec<u8>)>>>, // sequence_id -> (message, sent_time, retransmission_count, target_addr, encoded_message)
    receiver_channel: mpsc::Receiver<(PacketHeader, HandshakeMessage, SocketAddr)>, // Channel for receiving messages from the processing task
    out_of_order_buffer: Arc<Mutex<HashMap<u32, (PacketHeader, HandshakeMessage, SocketAddr)>>>, // sequence_id -> (header, message, sender_addr)
    pub handshake_state: HandshakeState,
    pub eq_polling_started: bool,
    pub capabilities: Option<Arc<crate::networking::session::Capabilities>>,
    pub udp_port: u16,
    pub proxy_settings: Option<crate::ui::proxy::ProxySettings>,
    /// Performance settings for dynamic network configuration
    performance_settings: Option<PerformanceSettingsHandle>,
    
    /// Event channels for sending events to the application
    chat_event_sender: Option<mpsc::UnboundedSender<ChatEvent>>,
    object_update_sender: Option<mpsc::UnboundedSender<ObjectUpdateEvent>>,
    agent_movement_sender: Option<mpsc::UnboundedSender<AgentMovementCompleteEvent>>,
    health_update_sender: Option<mpsc::UnboundedSender<HealthUpdateEvent>>,
    avatar_update_sender: Option<mpsc::UnboundedSender<AvatarDataUpdateEvent>>,
    region_handshake_sender: Option<mpsc::UnboundedSender<RegionHandshakeEvent>>,
    connection_status_sender: Option<mpsc::UnboundedSender<ConnectionStatusEvent>>,
    keep_alive_sender: Option<mpsc::UnboundedSender<KeepAliveEvent>>,
    
    /// Command receiver for receiving commands from the application
    command_receiver: Option<mpsc::UnboundedReceiver<NetworkCommand>>,
    
    /// Shared agent state for dynamic updates.
    /// Update this from UI/game logic to change position, camera, controls, etc.:
    /// {
    ///     let mut state = circuit.agent_state.lock().unwrap();
    ///     state.position = new_position;
    ///     state.camera_at = new_camera_at;
    ///     state.camera_eye = new_camera_eye;
    ///     state.controls = new_controls;
    /// }
    pub agent_state: Arc<Mutex<AgentState>>,
    
    /// Handshake configuration for controlling timing and behavior (shared with UDP task)
    pub handshake_config: Arc<Mutex<HandshakeConfig>>,
    
    /// Coordination flag: UDP task sets this when RegionHandshakeReply is sent
    region_handshake_reply_sent: Arc<Mutex<bool>>,
}

impl Circuit {
    pub async fn new_with_transport(transport: Arc<Mutex<UdpTransport>>, agent_state: Arc<Mutex<AgentState>>) -> std::io::Result<Self> {
        Self::new_with_transport_and_settings(transport, agent_state, None).await
    }

    pub async fn new_with_transport_and_settings(
        transport: Arc<Mutex<UdpTransport>>, 
        agent_state: Arc<Mutex<AgentState>>,
        performance_settings: Option<PerformanceSettingsHandle>,
    ) -> std::io::Result<Self> {
        let (sender_channel_for_task, receiver_channel) = mpsc::channel(100);

        let unacked_messages_arc = Arc::new(Mutex::new(HashMap::<u32, (HandshakeMessage, Instant, u32, SocketAddr, Vec<u8>)>::new()));
        let unacked_messages_arc_clone = Arc::clone(&unacked_messages_arc);
        let next_expected_sequence_number_arc = Arc::new(Mutex::new(1));
        let next_expected_sequence_number_arc_clone = Arc::clone(&next_expected_sequence_number_arc);
        let out_of_order_buffer_arc = Arc::new(Mutex::new(HashMap::<u32, (PacketHeader, HandshakeMessage, SocketAddr)>::new()));
        let out_of_order_buffer_arc_clone = Arc::clone(&out_of_order_buffer_arc);
        let transport_bg = Arc::clone(&transport);
        let performance_settings_clone = performance_settings.clone();
        
        // Create shared handshake config for UDP task
        let handshake_config_arc = Arc::new(Mutex::new(HandshakeConfig::default()));
        let handshake_config_for_task = Arc::clone(&handshake_config_arc);
        
        // Create shared coordination flag for RegionHandshakeReply
        let region_handshake_reply_sent_arc = Arc::new(Mutex::new(false));
        let region_handshake_reply_sent_for_task = Arc::clone(&region_handshake_reply_sent_arc);

        // Spawn the UDP receive/retransmit task  
        tokio::spawn(async move {
            // Use the trait object for send/recv
            let mut buf = vec![0; 1024];
            loop {
                let transport_locked = transport_bg.lock().await;
                tokio::select! {
                    Ok((len, addr)) = transport_locked.recv_from(&mut buf) => {
                        info!("[UDP RX] 📥 Received {} bytes from {}: {:02X?}", len, addr, &buf[..len]);
                        if let Ok((header, message)) = SLMessageCodec::decode_handshake(&buf[..len]) {
                            info!("[UDP RX] 🔍 Decoded message: {:?} (seq: {}) from {}", message, header.sequence_id, addr);
                            match &message {
                                HandshakeMessage::UseCircuitCodeReply(success) => {
                                    info!("[HANDSHAKE] 📨 Step 1 Reply: UseCircuitCodeReply success={}", success);
                                }
                                HandshakeMessage::AgentMovementComplete { .. } => {
                                    info!("[HANDSHAKE] 📨 Step 2 Reply: AgentMovementComplete received!");
                                }
                                HandshakeMessage::RegionHandshake { .. } => {
                                    info!("[HANDSHAKE] 📨 Step 3: RegionHandshake received - handling in isolation for immediate response");
                                    // Handle RegionHandshake in isolation because:
                                    // 1. Needs immediate response (can't wait for main message loop)
                                    // 2. Main Circuit doesn't have real agent_id/session_id yet
                                    // 3. Early handshake timing requirements
                                    
                                    let agent_id = uuid::Uuid::nil(); // Will be updated via coordination mechanism
                                    let session_id = uuid::Uuid::nil(); // Will be updated via coordination mechanism  
                                    let circuit_code = 0; // Will be updated via coordination mechanism
                                    let flags = 5; // Default flags value like official viewer
                                    
                                    let reply_message = HandshakeMessage::RegionHandshakeReply {
                                        agent_id: agent_id.to_string(),
                                        session_id: session_id.to_string(),
                                        flags,
                                    };
                                    
                                    let reply_header = PacketHeader { 
                                        sequence_id: 100, // TODO: Use proper sequence counter
                                        flags: 0x40 // Reliable delivery
                                    };
                                    
                                    // Apply configured delay before sending RegionHandshakeReply
                                    Self::apply_handshake_delay(&handshake_config_for_task).await;
                                    
                                    match SLMessageCodec::encode_handshake(&reply_header, &reply_message) {
                                        Ok(reply_packet) => {
                                            info!("[HANDSHAKE] 📤 Step 4/7: Sending RegionHandshakeReply from UDP task (seq: {})", reply_header.sequence_id);
                                            let _ = transport_locked.send_to(&reply_packet, &addr).await;
                                            
                                            // Set coordination flag to notify main Circuit that RegionHandshakeReply was sent
                                            *region_handshake_reply_sent_for_task.lock().await = true;
                                            info!("[HANDSHAKE] 🔗 Set coordination flag: RegionHandshakeReply sent from UDP task");
                                        },
                                        Err(e) => {
                                            tracing::warn!("[HANDSHAKE] ❌ Failed to encode RegionHandshakeReply: {}", e);
                                        }
                                    }
                                }
                                _ => {}
                            }
                            match message {
                                HandshakeMessage::Ack { sequence_id } => {
                                    let mut unacked_messages = unacked_messages_arc_clone.lock().await;
                                    unacked_messages.remove(&sequence_id);
                                }
                                HandshakeMessage::KeepAlive => {
                                    // Do not send an ACK for KeepAlive!
                                    // Just log or handle as needed.
                                }
                                received_message => {
                                    let mut messages_to_send = Vec::new();
                                    {
                                        let mut current_expected_seq = next_expected_sequence_number_arc_clone.lock().await;
                                        let mut out_of_order_buffer = out_of_order_buffer_arc_clone.lock().await;

                                        if header.sequence_id == *current_expected_seq {
                                            messages_to_send.push((header.clone(), received_message.clone(), addr));
                                            *current_expected_seq += 1;
                                            while let Some((h, m, a)) = out_of_order_buffer.remove(&*current_expected_seq) {
                                                messages_to_send.push((h, m, a));
                                                *current_expected_seq += 1;
                                            }
                                        } else if header.sequence_id > *current_expected_seq {
                                            out_of_order_buffer.insert(header.sequence_id, (header.clone(), received_message.clone(), addr));
                                        } else {
                                            tracing::debug!("Discarding duplicate or old packet: {:?}", header);
                                        }
                                    }
                                    // Only send ACK if not KeepAlive and if message requires reliable delivery
                                    if !matches!(received_message, HandshakeMessage::KeepAlive) && (header.flags & 0x40) != 0 {
                                        let ack_message = HandshakeMessage::Ack { sequence_id: header.sequence_id };
                                        let ack_header = PacketHeader { 
                                            sequence_id: 0, // ACKs don't need sequence numbers
                                            flags: 0x00 // Not reliable, not zerocoded
                                        };
                                        
                                        // Use SL codec for proper ACK encoding
                                        match SLMessageCodec::encode_handshake(&ack_header, &ack_message) {
                                            Ok(ack_packet) => {
                                                tracing::debug!("[UDP TX] 📤 Sending ACK for seq {} to {}: {:02X?}", 
                                                    header.sequence_id, addr, &ack_packet[..std::cmp::min(ack_packet.len(), 20)]);
                                                let _ = transport_locked.send_to(&ack_packet, &addr).await;
                                            },
                                            Err(e) => {
                                                tracing::warn!("[UDP TX] ❌ Failed to encode ACK packet: {}", e);
                                            }
                                        }
                                    }
                                    for (h, m, a) in messages_to_send {
                                        let _ = sender_channel_for_task.send((h, m, a)).await;
                                    }
                                }
                            }
                        } else {
                            // Try legacy UseCircuitCode parsing
                            let pkt = &buf[..len];
                            // Legacy UseCircuitCode: flags(1) + packet_id(4) + offset(1) + msgnum(4) + circuit_code(4) + session_id(16) + agent_id(16) = 46 bytes
                            if pkt.len() == 46 && pkt[0] & 0x40 != 0 && pkt[6..10] == [0xFF, 0xFF, 0x00, 0x03] {
                                let packet_id = u32::from_le_bytes([pkt[1], pkt[2], pkt[3], pkt[4]]);
                                let circuit_code = u32::from_le_bytes([pkt[10], pkt[11], pkt[12], pkt[13]]);
                                let session_id = uuid::Uuid::from_bytes([pkt[14], pkt[15], pkt[16], pkt[17], pkt[18], pkt[19], pkt[20], pkt[21], pkt[22], pkt[23], pkt[24], pkt[25], pkt[26], pkt[27], pkt[28], pkt[29]]);
                                let agent_id = uuid::Uuid::from_bytes([pkt[30], pkt[31], pkt[32], pkt[33], pkt[34], pkt[35], pkt[36], pkt[37], pkt[38], pkt[39], pkt[40], pkt[41], pkt[42], pkt[43], pkt[44], pkt[45]]);
                                let message = HandshakeMessage::UseCircuitCode {
                                    agent_id: agent_id.to_string(),
                                    session_id: session_id.to_string(),
                                    circuit_code,
                                };
                                let header = PacketHeader { sequence_id: packet_id, flags: pkt[0] };
                                println!("[UDP RX] Parsed legacy UseCircuitCode: circuit_code={}, session_id={}, agent_id={}, seq={}", circuit_code, session_id, agent_id, packet_id);
                                let _ = sender_channel_for_task.send((header, message, addr)).await;
                            } else {
                                println!("[UDP RX] Failed to decode UDP packet from {}: {:02X?}", addr, &buf[..len]);
                            }
                        }
                    },
                    _ = time::sleep(Duration::from_millis(Self::get_retransmission_timeout_ms(&performance_settings_clone))) => {
                        let mut messages_to_retransmit = Vec::new();
                        let mut lost_messages = Vec::new();
                        let mut unacked_messages = unacked_messages_arc_clone.lock().await;
                        let max_retransmissions = Self::get_max_retransmissions(&performance_settings_clone);
                        let timeout_ms = Self::get_retransmission_timeout_ms(&performance_settings_clone);
                        unacked_messages.retain(|&seq_id, (_message, sent_time, retransmission_count, target_addr, encoded_message)| {
                            if sent_time.elapsed() > Duration::from_millis(timeout_ms) {
                                if *retransmission_count < max_retransmissions {
                                    messages_to_retransmit.push((seq_id, target_addr.clone(), encoded_message.clone()));
                                    *sent_time = Instant::now();
                                    *retransmission_count += 1;
                                    true
                                } else {
                                    lost_messages.push(seq_id);
                                    false
                                }
                            } else {
                                true
                            }
                        });
                        for (seq_id, target_addr, encoded_message) in messages_to_retransmit {
                            let _ = transport_locked.send_to(&encoded_message, &target_addr).await;
                            tracing::debug!("Retransmitting message {} to {}.", seq_id, target_addr);
                        }
                        for seq_id in lost_messages {
                            tracing::warn!("Message {} lost after {} retransmissions.", seq_id, max_retransmissions);
                        }
                    }
                }
            }
        });

        Ok(Self {
            transport,
            next_sequence_number: 1,
            next_expected_sequence_number: Arc::new(Mutex::new(1)),
            unacked_messages: unacked_messages_arc,
            receiver_channel,
            out_of_order_buffer: Arc::new(Mutex::new(HashMap::new())),
            handshake_state: HandshakeState::NotStarted,
            eq_polling_started: false,
            capabilities: None,
            udp_port: 0,
            proxy_settings: None,
            performance_settings,
            
            // Initialize event channels as None - will be set up by the app
            chat_event_sender: None,
            object_update_sender: None,
            agent_movement_sender: None,
            health_update_sender: None,
            avatar_update_sender: None,
            region_handshake_sender: None,
            connection_status_sender: None,
            keep_alive_sender: None,
            command_receiver: None,
            
            agent_state,
            handshake_config: handshake_config_arc,
            region_handshake_reply_sent: region_handshake_reply_sent_arc,
        })
    }

    /// Set handshake configuration
    pub async fn set_handshake_config(&mut self, config: HandshakeConfig) {
        let delay_ms = config.region_handshake_reply_delay_ms;
        *self.handshake_config.lock().await = config;
        info!("[HANDSHAKE] 🔧 Updated handshake config: RegionHandshakeReply delay = {}ms", delay_ms);
    }
    
    /// Get current handshake configuration (for testing/debugging)
    pub async fn get_handshake_config(&self) -> HandshakeConfig {
        self.handshake_config.lock().await.clone()
    }
    
    /// Set RegionHandshakeReply delay specifically (convenience method)
    pub async fn set_region_handshake_delay(&mut self, delay_ms: u64) {
        let mut config = self.handshake_config.lock().await;
        config.region_handshake_reply_delay_ms = delay_ms;
        info!("[HANDSHAKE] 🔧 Set RegionHandshakeReply delay to {}ms", delay_ms);
    }
    
    /// Check if UDP task sent RegionHandshakeReply and continue handshake if needed
    pub async fn check_and_continue_handshake(
        &mut self,
        agent_id: uuid::Uuid,
        session_id: uuid::Uuid,
        circuit_code: u32,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
        throttle: [f32; 7],
        flags: u32,
        controls: u32,
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        target_addr: &SocketAddr,
    ) -> bool {
        let should_continue = {
            let mut flag = self.region_handshake_reply_sent.lock().await;
            if *flag && self.handshake_state == HandshakeState::SentCompleteAgentMovement {
                info!("[HANDSHAKE] 🔗 Detected UDP task sent RegionHandshakeReply, continuing handshake progression");
                *flag = false; // Reset flag
                true
            } else {
                false
            }
        }; // Lock is dropped here
        
        if should_continue {
            // Update state to reflect that RegionHandshakeReply was sent
            self.handshake_state = HandshakeState::SentRegionHandshakeReply;
            
            // FIX: Use the generated messages path to ensure correct AgentThrottle encoding.
            // This resolves the "variable value is not set" error for the 'Throttles' field.
            info!("[HANDSHAKE] 🔗 Handshake coordination advancing with generated messages path.");
            if let Err(e) = self.advance_handshake_with_generated_messages(
                agent_id, session_id, circuit_code, target_addr, position, look_at, throttle, flags, controls, camera_at, camera_eye
            ).await {
                warn!("[HANDSHAKE] ❌ Error advancing handshake with generated messages: {}", e);
            }
            true
        } else {
            false
        }
    }

    /// Helper method to apply configured delay before sending RegionHandshakeReply
    async fn apply_region_handshake_reply_delay(&self) {
        Self::apply_handshake_delay(&self.handshake_config).await;
    }
    
    /// Static helper to apply delay from a shared HandshakeConfig (for use in UDP task)
    async fn apply_handshake_delay(config_arc: &Arc<Mutex<HandshakeConfig>>) {
        let config = config_arc.lock().await;
        if config.region_handshake_reply_delay_ms > 0 {
            let delay = Duration::from_millis(config.region_handshake_reply_delay_ms);
            info!("[HANDSHAKE] ⏱️  Applying {}ms delay before RegionHandshakeReply (BUG-233107 workaround)", 
                  config.region_handshake_reply_delay_ms);
            drop(config); // Release lock before sleeping
            tokio::time::sleep(delay).await;
        }
    }

    pub async fn send_message(&mut self, message: &HandshakeMessage, target: &SocketAddr) -> io::Result<usize> {
        // Set appropriate flags based on message type
        let flags = match message {
            HandshakeMessage::UseCircuitCode { .. } => 0x40, // RELIABLE
            HandshakeMessage::CompleteAgentMovement { .. } => 0x40, // RELIABLE  
            HandshakeMessage::RegionHandshakeReply { .. } => 0x40, // RELIABLE
            HandshakeMessage::AgentThrottle { .. } => 0x40, // RELIABLE
            HandshakeMessage::AgentUpdate { .. } => 0x00, // UNRELIABLE (high frequency)
            HandshakeMessage::Ack { .. } => 0x00, // UNRELIABLE
            _ => 0x00, // Default to unreliable
        };
        
        let header = PacketHeader {
            sequence_id: self.next_sequence_number,
            flags,
        };
        self.next_sequence_number += 1;
        
        // Use the new encoding method
        let encoded = SLMessageCodec::encode_handshake(&header, message)?;
        
        // Store reliable messages for retransmission
        if flags & 0x40 != 0 { // RELIABLE flag
            let mut unacked_messages = self.unacked_messages.lock().await;
            unacked_messages.insert(
                header.sequence_id,
                (message.clone(), Instant::now(), 0, target.clone(), encoded.clone()),
            );
        }
        
        info!("[UDP TX] 📤 Sending {} bytes to {} (seq: {}, flags: 0x{:02X}): {:02X?}", 
              encoded.len(), target, header.sequence_id, flags, &encoded[..std::cmp::min(16, encoded.len())]);
        info!("[UDP TX] 🔍 Message type: {:?}", message);
        
        let transport = self.transport.lock().await;
        transport.send_to(&encoded, target).await
    }

    /// Send a message using the generated Message enum (Phase 3 implementation)
    pub async fn send_generated_message(&mut self, message: &Message, reliable: bool, target: &SocketAddr) -> io::Result<usize> {
        let flags = if reliable { 0x40 } else { 0x00 }; // RELIABLE flag
        
        let header = PacketHeader {
            sequence_id: self.next_sequence_number,
            flags,
        };
        self.next_sequence_number += 1;
        
        // Use the generated message codec
        let encoded = MessageCodec::encode(&header, message)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        // Store reliable messages for retransmission (placeholder for now)
        if reliable {
            // TODO: Update unacked_messages to support Message enum
            info!("[UDP TX] 📝 Note: Reliable message storage not yet implemented for generated messages");
        }
        
        info!("[UDP TX] 📤 Sending {} bytes to {} (seq: {}, flags: 0x{:02X}) [Generated Message]", 
              encoded.len(), target, header.sequence_id, flags);
        info!("[UDP TX] 🔍 Generated message type: {:?}", message);
        
        let transport = self.transport.lock().await;
        transport.send_to(&encoded, target).await
    }

    pub async fn recv_message(&mut self) -> io::Result<(PacketHeader, HandshakeMessage, SocketAddr)> {
        self.receiver_channel.recv().await.ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Circuit receive channel closed"))
    }

    /// Get retransmission timeout based on performance settings
    fn get_retransmission_timeout_ms(performance_settings: &Option<PerformanceSettingsHandle>) -> u64 {
        if let Some(settings) = performance_settings {
            if let Ok(settings) = settings.read() {
                return settings.networking.connection_timeout_ms as u64 / 10; // Fraction of total timeout
            }
        }
        DEFAULT_RETRANSMISSION_TIMEOUT_MS
    }

    /// Get maximum retransmissions based on performance settings  
    fn get_max_retransmissions(performance_settings: &Option<PerformanceSettingsHandle>) -> u32 {
        if let Some(settings) = performance_settings {
            if let Ok(settings) = settings.read() {
                return settings.networking.retry_attempts;
            }
        }
        DEFAULT_MAX_RETRANSMISSIONS
    }

    /// Update performance settings for dynamic network configuration
    pub fn update_performance_settings(&mut self, new_settings: PerformanceSettingsHandle) {
        self.performance_settings = Some(new_settings);
    }

    /// Helper method to convert handshake data to generated UseCircuitCode message
    pub fn create_use_circuit_code_message(&self, agent_id: uuid::Uuid, session_id: uuid::Uuid, circuit_code: u32) -> Message {
        use crate::networking::protocol::messages::{UseCircuitCode};
        
        Message::UseCircuitCode(UseCircuitCode {
            code: circuit_code,
            session_id,
            id: agent_id.as_bytes().to_vec(), // Convert agent_id to Vec<u8>
        })
    }

    /// Helper method to create CompleteAgentMovement message  
    pub fn create_complete_agent_movement_message(&self, agent_id: uuid::Uuid, session_id: uuid::Uuid, circuit_code: u32) -> Message {
        use crate::networking::protocol::messages::{CompleteAgentMovement};
        
        Message::CompleteAgentMovement(CompleteAgentMovement {
            agent_id,
            session_id, 
            circuit_code,
        })
    }

    /// Helper method to create RegionHandshakeReply message
    pub fn create_region_handshake_reply_message(&self, agent_id: uuid::Uuid, session_id: uuid::Uuid, flags: u32) -> Message {
        use crate::networking::protocol::messages::{RegionHandshakeReply};
        
        Message::RegionHandshakeReply(RegionHandshakeReply {
            agent_id,
            session_id,
            flags,
        })
    }

    /// Helper method to create AgentThrottle message
    pub fn create_agent_throttle_message(&self, agent_id: uuid::Uuid, session_id: uuid::Uuid, circuit_code: u32, throttle: [f32; 7]) -> Message {
        use crate::networking::protocol::messages::{AgentThrottle};
        
        // Convert throttle values to bytes (Second Life protocol format - big-endian)
        let mut throttle_bytes = Vec::new();
        for val in throttle {
            throttle_bytes.extend_from_slice(&val.to_be_bytes());
        }
        
        Message::AgentThrottle(AgentThrottle {
            agent_id,
            session_id,
            circuit_code,
            gen_counter: 0, // Default generation counter
            throttles: throttle_bytes,
        })
    }

    /// Helper method to create AgentUpdate message
    pub fn create_agent_update_message(&self, agent_id: uuid::Uuid, session_id: uuid::Uuid, position: (f32, f32, f32), camera_at: (f32, f32, f32), camera_eye: (f32, f32, f32), controls: u32) -> Message {
        use crate::networking::protocol::messages::{AgentUpdate};
        
        Message::AgentUpdate(AgentUpdate {
            agent_id,
            session_id,
            body_rotation: [0.0, 0.0, 0.0, 1.0], // Default quaternion (no rotation)
            head_rotation: [0.0, 0.0, 0.0, 1.0], // Default quaternion
            state: 0, // Default state
            camera_center: [camera_eye.0, camera_eye.1, camera_eye.2], // Convert tuple to array
            camera_at_axis: [camera_at.0, camera_at.1, camera_at.2],
            camera_left_axis: [0.0, 1.0, 0.0], // Default left axis
            camera_up_axis: [0.0, 0.0, 1.0], // Default up axis
            far: 512.0, // Default far plane distance
            control_flags: controls,
            flags: 0, // Default flags
        })
    }

    /// Set up event channels for communication with the application
    pub fn setup_event_channels(
        &mut self,
        chat_sender: mpsc::UnboundedSender<ChatEvent>,
        object_update_sender: mpsc::UnboundedSender<ObjectUpdateEvent>,
        agent_movement_sender: mpsc::UnboundedSender<AgentMovementCompleteEvent>,
        health_update_sender: mpsc::UnboundedSender<HealthUpdateEvent>,
        avatar_update_sender: mpsc::UnboundedSender<AvatarDataUpdateEvent>,
        region_handshake_sender: mpsc::UnboundedSender<RegionHandshakeEvent>,
        connection_status_sender: mpsc::UnboundedSender<ConnectionStatusEvent>,
        keep_alive_sender: mpsc::UnboundedSender<KeepAliveEvent>,
        command_receiver: mpsc::UnboundedReceiver<NetworkCommand>,
    ) {
        self.chat_event_sender = Some(chat_sender);
        self.object_update_sender = Some(object_update_sender);
        self.agent_movement_sender = Some(agent_movement_sender);
        self.health_update_sender = Some(health_update_sender);
        self.avatar_update_sender = Some(avatar_update_sender);
        self.region_handshake_sender = Some(region_handshake_sender);
        self.connection_status_sender = Some(connection_status_sender);
        self.keep_alive_sender = Some(keep_alive_sender);
        self.command_receiver = Some(command_receiver);
    }

    /// Process commands from the application
    pub async fn process_commands(&mut self, target_addr: &SocketAddr) -> io::Result<()> {
        // Collect all pending commands first to avoid borrowing conflicts
        let mut commands = Vec::new();
        if let Some(ref mut receiver) = self.command_receiver {
            while let Ok(command) = receiver.try_recv() {
                commands.push(command);
            }
        }

        // Process collected commands
        for command in commands {
            match command {
                NetworkCommand::SendChat { message, channel, chat_type } => {
                    let msg = HandshakeMessage::ChatFromViewer {
                        message,
                        channel: channel.to_string(),
                    };
                    let _ = self.send_message(&msg, target_addr).await;
                },
                NetworkCommand::SendAgentUpdate { position, camera_at, camera_eye, controls } => {
                    // Update the shared agent state
                    {
                        let mut state = self.agent_state.lock().await;
                        state.position = position;
                        state.camera_at = camera_at;
                        state.camera_eye = camera_eye;
                        state.controls = controls;
                    }
                    // Note: AgentUpdate is sent automatically by the periodic task
                },
                NetworkCommand::RequestObject { id } => {
                    // TODO: Implement object request message
                    tracing::debug!("Object request not yet implemented: {}", id);
                },
                NetworkCommand::RequestTexture { texture_id } => {
                    // TODO: Implement texture request message
                    tracing::debug!("Texture request not yet implemented: {}", texture_id);
                },
                NetworkCommand::SendThrottle { throttle } => {
                    let msg = HandshakeMessage::AgentThrottle {
                        agent_id: "unknown".to_string(), // TODO: Use actual agent ID
                        session_id: "unknown".to_string(), // TODO: Use actual session ID
                        circuit_code: 0, // TODO: Use actual circuit code
                        throttle,
                    };
                    let _ = self.send_message(&msg, target_addr).await;
                },
                NetworkCommand::Logout => {
                    let msg = HandshakeMessage::Logout;
                    let _ = self.send_message(&msg, target_addr).await;
                },
                NetworkCommand::SendRawMessage { message } => {
                    let _ = self.send_message(&message, target_addr).await;
                },
            }
        }
        Ok(())
    }

    /// Dispatch an incoming message to the appropriate event channel
    async fn dispatch_event(&self, message: &HandshakeMessage) {
        match message {
            HandshakeMessage::ChatFromSimulator { sender, message: msg, channel } => {
                if let Some(sender_channel) = &self.chat_event_sender {
                    let event = ChatEvent::new(
                        sender.clone(),
                        uuid::Uuid::nil(), // TODO: Extract sender UUID if available
                        msg.clone(),
                        0, // TODO: Parse channel
                        1, // TODO: Parse chat type
                    );
                    let _ = sender_channel.send(event);
                }
            },
            HandshakeMessage::AgentMovementComplete { agent_id, session_id } => {
                if let Some(sender_channel) = &self.agent_movement_sender {
                    let event = AgentMovementCompleteEvent {
                        agent_id: uuid::Uuid::parse_str(agent_id).unwrap_or(uuid::Uuid::nil()),
                        session_id: uuid::Uuid::parse_str(session_id).unwrap_or(uuid::Uuid::nil()),
                        timestamp: std::time::SystemTime::now(),
                    };
                    let _ = sender_channel.send(event);
                }
            },
            HandshakeMessage::HealthMessage => {
                if let Some(sender_channel) = &self.health_update_sender {
                    let event = HealthUpdateEvent {
                        health: 100.0, // TODO: Extract actual health value
                        timestamp: std::time::SystemTime::now(),
                    };
                    let _ = sender_channel.send(event);
                }
            },
            HandshakeMessage::AgentDataUpdate { agent_id } => {
                if let Some(sender_channel) = &self.avatar_update_sender {
                    let event = AvatarDataUpdateEvent {
                        agent_id: uuid::Uuid::parse_str(agent_id).unwrap_or(uuid::Uuid::nil()),
                        firstname: "Unknown".to_string(), // TODO: Extract actual data
                        lastname: "User".to_string(),
                        group_title: "".to_string(),
                        timestamp: std::time::SystemTime::now(),
                    };
                    let _ = sender_channel.send(event);
                }
            },
            HandshakeMessage::RegionHandshake { region_name, region_id, region_flags, water_height, sim_access } => {
                if let Some(sender_channel) = &self.region_handshake_sender {
                    let event = RegionHandshakeEvent {
                        region_name: region_name.clone(),
                        region_id: *region_id,
                        region_flags: *region_flags,
                        water_height: *water_height,
                        sim_access: *sim_access,
                        timestamp: std::time::SystemTime::now(),
                    };
                    let _ = sender_channel.send(event);
                }
            },
            HandshakeMessage::KeepAlive => {
                if let Some(sender_channel) = &self.keep_alive_sender {
                    let event = KeepAliveEvent {
                        timestamp: std::time::SystemTime::now(),
                    };
                    let _ = sender_channel.send(event);
                }
            },
            _ => {
                // For unhandled message types, just log or ignore
                tracing::debug!("Unhandled message type in dispatcher: {:?}", message);
            }
        }
    }

    /// Main receive loop for processing incoming UDP messages and enforcing handshake state machine
    pub async fn run_receive_loop(&mut self,
        agent_id: uuid::Uuid,
        session_id: uuid::Uuid,
        circuit_code: u32,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
        throttle: [f32; 7],
        _flags: u32,
        controls: u32,
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        target_addr: &SocketAddr,
    ) {
        let mut coordination_check_interval = tokio::time::interval(Duration::from_millis(100));
        
        loop {
            tokio::select! {
                // Check for UDP task coordination (RegionHandshakeReply sent)
                _ = coordination_check_interval.tick() => {
                    self.check_and_continue_handshake(
                        agent_id, session_id, circuit_code, position, look_at, 
                        throttle, _flags, controls, camera_at, camera_eye, target_addr
                    ).await;
                }
                // Handle incoming network messages
                message_result = self.recv_message() => {
                    match message_result {
                        Ok((header, message, addr)) => {
                            self.handle_incoming_message(
                                header,
                                message,
                                addr,
                                agent_id,
                                session_id,
                                circuit_code,
                                position,
                                look_at,
                                throttle,
                                _flags,
                                controls,
                                camera_at,
                                camera_eye,
                                target_addr
                            ).await;
                        }
                        Err(e) => {
                            eprintln!("[UDP RX] Error receiving message: {}", e);
                            break;
                        }
                    }
                }
                // Process commands from the application
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    if let Err(e) = self.process_commands(target_addr).await {
                        tracing::error!("Error processing commands: {}", e);
                    }
                }
            }
        }
    }

    /// Experimental handshake method using generated Message enum (Phase 3 implementation)
    /// This demonstrates the transition to the full protocol implementation
    pub async fn advance_handshake_with_generated_messages(
        &mut self,
        agent_id: uuid::Uuid,
        session_id: uuid::Uuid,
        circuit_code: u32,
        target_addr: &SocketAddr,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
        throttle: [f32; 7],
        flags: u32,
        controls: u32,
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
    ) -> io::Result<()> {
        use tracing::info;
        
        match self.handshake_state {
            HandshakeState::NotStarted => {
                info!("[HANDSHAKE] 🆕 Using Generated Messages: Step 1/7: Sending UseCircuitCode");
                let message = self.create_use_circuit_code_message(agent_id, session_id, circuit_code);
                self.send_generated_message(&message, true, target_addr).await?;
                self.handshake_state = HandshakeState::SentUseCircuitCode;
                info!("[HANDSHAKE] ✅ Generated message transition: NotStarted -> SentUseCircuitCode");
            }
            HandshakeState::SentUseCircuitCode => {
                info!("[HANDSHAKE] 🆕 Using Generated Messages: Step 2/7: Sending CompleteAgentMovement");
                let message = self.create_complete_agent_movement_message(agent_id, session_id, circuit_code);
                self.send_generated_message(&message, true, target_addr).await?;
                self.handshake_state = HandshakeState::SentCompleteAgentMovement;
                info!("[HANDSHAKE] ✅ Generated message transition: SentUseCircuitCode -> SentCompleteAgentMovement");
            }
            HandshakeState::SentCompleteAgentMovement => {
                info!("[HANDSHAKE] 🆕 Generated Messages: Step 3/7: Waiting for RegionHandshake from server...");
                // This state waits for RegionHandshake message which will be handled in the incoming message handler
                return Ok(());
            }
            HandshakeState::ReceivedRegionHandshake => {
                info!("[HANDSHAKE] 🆕 Using Generated Messages: Step 4/7: Sending RegionHandshakeReply");
                
                // Apply configured delay before sending RegionHandshakeReply
                self.apply_region_handshake_reply_delay().await;
                
                let message = self.create_region_handshake_reply_message(agent_id, session_id, flags);
                self.send_generated_message(&message, true, target_addr).await?;
                self.handshake_state = HandshakeState::SentRegionHandshakeReply;
                info!("[HANDSHAKE] ✅ Generated message transition: ReceivedRegionHandshake -> SentRegionHandshakeReply");
            }
            HandshakeState::SentRegionHandshakeReply => {
                info!("[HANDSHAKE] 🆕 Using Generated Messages: Step 5/7: Sending AgentThrottle");
                let message = self.create_agent_throttle_message(agent_id, session_id, circuit_code, throttle);
                self.send_generated_message(&message, true, target_addr).await?;
                self.handshake_state = HandshakeState::SentAgentThrottle;
                info!("[HANDSHAKE] ✅ Generated message transition: SentRegionHandshakeReply -> SentAgentThrottle");
            }
            HandshakeState::SentAgentThrottle => {
                info!("[HANDSHAKE] 🆕 Using Generated Messages: Step 6/7: Sending first AgentUpdate");
                let message = self.create_agent_update_message(agent_id, session_id, position, camera_at, camera_eye, controls);
                self.send_generated_message(&message, false, target_addr).await?; // AgentUpdate is unreliable
                self.handshake_state = HandshakeState::SentFirstAgentUpdate;
                info!("[HANDSHAKE] ✅ Generated message transition: SentAgentThrottle -> SentFirstAgentUpdate");
            }
            HandshakeState::SentFirstAgentUpdate => {
                info!("[HANDSHAKE] 🆕 Using Generated Messages: Step 7/7: Handshake complete!");
                self.handshake_state = HandshakeState::HandshakeComplete;
                info!("[HANDSHAKE] ✅ Generated message transition: SentFirstAgentUpdate -> HandshakeComplete");
                
                // Start 10-second shutdown timer after successful authentication
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    info!("[SHUTDOWN] 🔴 Auto-shutdown timer reached (Generated Messages) - terminating application for debug analysis");
                    std::process::exit(0);
                });
            }
            HandshakeState::HandshakeComplete => {
                info!("[HANDSHAKE] 🆕 Generated Messages: Handshake already complete");
                return Ok(());
            }
        }
        Ok(())
    }

    pub async fn advance_handshake(
        &mut self,
        agent_id: uuid::Uuid,
        session_id: uuid::Uuid,
        circuit_code: u32,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
        throttle: [f32; 7],
        flags: u32,
        controls: u32,
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        target_addr: &SocketAddr,
    ) {
        use tracing::{info, warn};
        
        // Log handshake progress summary
        if self.handshake_state == HandshakeState::NotStarted {
            info!("[HANDSHAKE] 🚀 Starting authentication handshake sequence");
            info!("[HANDSHAKE] 📊 Flow: NotStarted → UseCircuitCode → CompleteAgentMovement → [RegionHandshake] → RegionHandshakeReply → AgentThrottle → AgentUpdate → Complete");
        }
        
        match self.handshake_state {
            HandshakeState::NotStarted => {
                info!("[HANDSHAKE] 📤 Step 1/7: Sending UseCircuitCode");
                info!("[HANDSHAKE] 📋 Agent ID: {}, Session ID: {}, Circuit Code: {}", agent_id, circuit_code, target_addr);
                let message = HandshakeMessage::UseCircuitCode {
                    agent_id: agent_id.to_string(),
                    session_id: session_id.to_string(),
                    circuit_code,
                };
                let _ = self.send_message(&message, target_addr).await;
                self.handshake_state = HandshakeState::SentUseCircuitCode;
                info!("[HANDSHAKE] ✅ State transition: NotStarted -> SentUseCircuitCode");
            }
            HandshakeState::SentUseCircuitCode => {
                info!("[HANDSHAKE] 📤 Step 2/7: Sending CompleteAgentMovement");
                info!("[HANDSHAKE] 📍 Position: {:?}, Look At: {:?}", position, look_at);
                let message = HandshakeMessage::CompleteAgentMovement {
                    agent_id: agent_id.to_string(),
                    session_id: session_id.to_string(),
                    circuit_code,
                    position,
                    look_at,
                };
                let _ = self.send_message(&message, target_addr).await;
                self.handshake_state = HandshakeState::SentCompleteAgentMovement;
                info!("[HANDSHAKE] ✅ State transition: SentUseCircuitCode -> SentCompleteAgentMovement");
            }
            HandshakeState::SentCompleteAgentMovement => {
                info!("[HANDSHAKE] ⏳ Step 3/7: Waiting for RegionHandshake from server...");
                warn!("[HANDSHAKE] ⚠️  advance_handshake called while waiting for RegionHandshake - this is expected");
                // Wait for RegionHandshake (IN)
                return;
            }
            HandshakeState::ReceivedRegionHandshake => {
                // This state is now handled directly in handle_incoming_message
                return;
            }
            HandshakeState::SentRegionHandshakeReply => {
                info!("[HANDSHAKE] 📤 Step 5/7: Sending AgentThrottle");
                info!("[HANDSHAKE] 🎛️  Throttle settings: {:?}", throttle);
                let message = HandshakeMessage::AgentThrottle {
                    agent_id: agent_id.to_string(),
                    session_id: session_id.to_string(),
                    circuit_code,
                    throttle,
                };
                let _ = self.send_message(&message, target_addr).await;
                self.handshake_state = HandshakeState::SentAgentThrottle;
                info!("[HANDSHAKE] ✅ State transition: SentRegionHandshakeReply -> SentAgentThrottle");
            }
            HandshakeState::SentAgentThrottle => {
                info!("[HANDSHAKE] 📤 Step 6/7: Sending first AgentUpdate");
                info!("[HANDSHAKE] 🚶 Agent state - Position: {:?}, Camera At: {:?}, Camera Eye: {:?}, Controls: {}", position, camera_at, camera_eye, controls);
                let message = HandshakeMessage::AgentUpdate {
                    agent_id: agent_id.to_string(),
                    session_id: session_id.to_string(),
                    position,
                    camera_at,
                    camera_eye,
                    controls,
                };
                let _ = self.send_message(&message, target_addr).await;
                self.handshake_state = HandshakeState::SentFirstAgentUpdate;
                info!("[HANDSHAKE] ✅ State transition: SentAgentThrottle -> SentFirstAgentUpdate");
            }
            HandshakeState::SentFirstAgentUpdate => {
                info!("[HANDSHAKE] ✅ Handshake complete! Starting EQ polling and periodic AgentUpdate.");
                info!("[HANDSHAKE] 🎯 Authentication successful - setting 10 second auto-shutdown timer");
                self.handshake_state = HandshakeState::HandshakeComplete;
                
                // Start 10-second shutdown timer after successful authentication
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    info!("[SHUTDOWN] 🔴 Auto-shutdown timer reached - terminating application for debug analysis");
                    std::process::exit(0);
                });
                if !self.eq_polling_started {
                    self.eq_polling_started = true;
                    // EQ polling
                    if let Some(ref caps) = self.capabilities {
                        let caps = caps.clone();
                        let udp_port = self.udp_port;
                        let proxy_settings = self.proxy_settings.clone();
                        tokio::spawn(async move {
                            let _ = crate::networking::session::poll_event_queue(
                                &caps,
                                udp_port,
                                proxy_settings.as_ref(),
                                |event_xml| {
                                    println!("[EQ] Event: {}", event_xml);
                                    // TODO: Forward to UI or state handler
                                }
                            ).await;
                        });
                    }
                    // Periodic AgentUpdate - need to rework this to use Circuit's send_message
                    // For now, keeping the old approach but this should be refactored to use
                    // a message channel to the main Circuit instance
                    let transport = self.transport.clone();
                    let agent_id = agent_id;
                    let session_id = session_id;
                    let agent_state = self.agent_state.clone();
                    let target_addr_clone = *target_addr;
                    tokio::spawn(async move {
                        let interval = tokio::time::Duration::from_millis(100);
                        let mut packet_counter = 1u32;
                        loop {
                            let (position, camera_at, camera_eye, controls) = {
                                let state = agent_state.lock().await;
                                (state.position, state.camera_at, state.camera_eye, state.controls)
                            };
                            
                            // Create AgentUpdate message directly
                            let message = HandshakeMessage::AgentUpdate {
                                agent_id: agent_id.to_string(),
                                session_id: session_id.to_string(),
                                position,
                                camera_at,
                                camera_eye,
                                controls,
                            };
                            
                            let header = PacketHeader {
                                sequence_id: packet_counter,
                                flags: 0x00, // UNRELIABLE for AgentUpdate
                            };
                            packet_counter += 1;
                            
                            if let Ok(encoded) = SLMessageCodec::encode_handshake(&header, &message) {
                                let transport = transport.lock().await;
                                let _ = transport.send_to(&encoded, &target_addr_clone).await;
                            }
                            
                            tokio::time::sleep(interval).await;
                        }
                    });
                }
            }
            HandshakeState::HandshakeComplete => {
                warn!("advance_handshake called after handshake is already complete");
                return;
            }
        }
    }

    // pub async fn disconnect_and_logout(&mut self, sim_addr: &SocketAddr) {
//     // Send Logout message
//     let _ = self.send_message(&HandshakeMessage::Logout, sim_addr).await;
//     // TODO: Add any additional cleanup if needed
// }

    pub async fn send_region_handshake_reply_with_seq(&mut self, agent_id: uuid::Uuid, session_id: uuid::Uuid, flags: u32, sequence_id: u32, addr: &SocketAddr) {
        // Create the message using the structured approach
        let message = HandshakeMessage::RegionHandshakeReply {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            flags,
        };
        
        // Use specific sequence ID for reply
        let header = PacketHeader {
            sequence_id,
            flags: 0x40, // RELIABLE
        };
        
        // Encode and send manually to use the specific sequence_id
        if let Ok(encoded) = SLMessageCodec::encode_handshake(&header, &message) {
            let transport = self.transport.lock().await;
            let _ = transport.send_to(&encoded, addr).await;
        }
        
        self.handshake_state = HandshakeState::SentRegionHandshakeReply;
    }

    pub async fn handle_incoming_message(&mut self, header: PacketHeader, message: HandshakeMessage, addr: SocketAddr,
        agent_id: uuid::Uuid,
        session_id: uuid::Uuid,
        circuit_code: u32,
        position: (f32, f32, f32),
        look_at: (f32, f32, f32),
        throttle: [f32; 7],
        flags: u32,
        controls: u32,
        camera_at: (f32, f32, f32),
        camera_eye: (f32, f32, f32),
        target_addr: &SocketAddr,
    ) {
        use tracing::info;
        
        // Always dispatch the event to application listeners first
        self.dispatch_event(&message).await;
        
        // Then handle handshake-specific logic
        match &message {
            HandshakeMessage::RegionHandshake { .. } => {
                info!("[HANDSHAKE] 🔍 RegionHandshake match arm entered. Current state: {:?}", self.handshake_state);
                if self.handshake_state < HandshakeState::SentRegionHandshakeReply {
                    info!("[HANDSHAKE] 📨 Step 3 Reply: RegionHandshake received from server! Advancing state.");
                    info!("[HANDSHAKE] 📤 Step 4/7: Sending RegionHandshakeReply (seq: {})", header.sequence_id);
                    
                    // Manually set the state to ensure we can send the reply and advance.
                    self.handshake_state = HandshakeState::ReceivedRegionHandshake;
                    
                    // Send reply with our own next sequence number (not echoing server's sequence number)
                    let reply_message = HandshakeMessage::RegionHandshakeReply {
                        agent_id: agent_id.to_string(),
                        session_id: session_id.to_string(),
                        flags,
                    };
                    
                    // Apply configured delay before sending RegionHandshakeReply
                    self.apply_region_handshake_reply_delay().await;
                    
                    let _ = self.send_message(&reply_message, &addr).await;
                    self.handshake_state = HandshakeState::SentRegionHandshakeReply;
                    
                    // Continue handshake progression, which will now send AgentThrottle
                    self.advance_handshake(agent_id, session_id, circuit_code, position, look_at, throttle, flags, controls, camera_at, camera_eye, target_addr).await;
                } else {
                    info!("[HANDSHAKE] Ignoring duplicate RegionHandshake message.");
                }
            }
            HandshakeMessage::KeepAlive => {
                // Per SL protocol and Hippolyzer, no response is required for KeepAlive packets.
                // Only log receipt for debugging.
                info!("[UDP] Received KeepAlive from {} (no response sent)", addr);
            }
            HandshakeMessage::AgentDataUpdate { .. } => {
                info!("[UDP] Received AgentDataUpdate from {}", addr);
                // ACK would be handled by the main loop, but specific logic could go here.
            }
            HandshakeMessage::HealthMessage => {
                info!("[UDP] Received HealthMessage from {}", addr);
                // ACK would be handled by the main loop.
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_disconnect_and_logout_sends_logout() {
        // This is a stub test; in a real test, you would mock UdpTransport and check that a Logout message is sent.
        // For now, just ensure the function runs without panicking.
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let proxy_settings: Option<&crate::ui::proxy::ProxySettings> = None;
        let transport = crate::networking::transport::UdpTransport::new(0, addr, proxy_settings).await.unwrap();
        let transport_arc = Arc::new(Mutex::new(transport));
        let agent_state = Arc::new(Mutex::new(AgentState {
            position: (0.0, 0.0, 0.0),
            camera_at: (0.0, 0.0, 0.0),
            camera_eye: (0.0, 0.0, 0.0),
            controls: 0,
        }));
        let mut circuit = Circuit::new_with_transport(transport_arc, agent_state).await.unwrap();
        // circuit.disconnect_and_logout(&addr).await; // Commented out as this method is not implemented yet
    }
}
