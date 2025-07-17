use crate::networking::protocol::messages::{PacketHeader, Message};
use crate::networking::protocol::codecs::MessageCodec;
use std::net::SocketAddr;
use std::io;
use std::collections::HashMap;
use tokio::time::{self, Instant, Duration};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::networking::transport::UdpTransport;

// Handshake State Machine for Second Life UDP Login
// Enforces strict ordering: UseCircuitCode -> CompleteAgentMovement -> (wait for RegionHandshake) -> RegionHandshakeReply -> AgentThrottle -> AgentUpdate -> HandshakeComplete
// Each handshake message is sent only once, and only after the previous step is complete. State is tracked per circuit/session.
// All handshake message sending is centralized in advance_handshake().
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

const RETRANSMISSION_TIMEOUT_MS: u64 = 200;
const MAX_RETRANSMISSIONS: u32 = 5;

// TODO: Refactor for proxy support. UdpSocketExt removed.
pub struct Circuit {
    transport: Arc<Mutex<UdpTransport>>,
    next_sequence_number: u32,
    next_expected_sequence_number: Arc<Mutex<u32>>,
    unacked_messages: Arc<Mutex<HashMap<u32, (Message, Instant, u32, SocketAddr, Vec<u8>)>>>, // sequence_id -> (message, sent_time, retransmission_count, target_addr, encoded_message)
    receiver_channel: mpsc::Receiver<(PacketHeader, Message, SocketAddr)>, // Channel for receiving messages from the processing task
    out_of_order_buffer: Arc<Mutex<HashMap<u32, (PacketHeader, Message, SocketAddr)>>>, // sequence_id -> (header, message, sender_addr)
    pub handshake_state: HandshakeState,
    pub eq_polling_started: bool,
    pub capabilities: Option<Arc<crate::networking::session::Capabilities>>,
    pub udp_port: u16,
    pub proxy_settings: Option<crate::ui::proxy::ProxySettings>,
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
}

impl Circuit {
    pub async fn new_with_transport(transport: Arc<Mutex<UdpTransport>>, agent_state: Arc<Mutex<AgentState>>) -> std::io::Result<Self> {
        let (sender_channel_for_task, receiver_channel) = mpsc::channel(100);

        let unacked_messages_arc = Arc::new(Mutex::new(HashMap::<u32, (Message, Instant, u32, SocketAddr, Vec<u8>)>::new()));
        let unacked_messages_arc_clone = Arc::clone(&unacked_messages_arc);
        let next_expected_sequence_number_arc = Arc::new(Mutex::new(1));
        let next_expected_sequence_number_arc_clone = Arc::clone(&next_expected_sequence_number_arc);
        let out_of_order_buffer_arc = Arc::new(Mutex::new(HashMap::<u32, (PacketHeader, Message, SocketAddr)>::new()));
        let out_of_order_buffer_arc_clone = Arc::clone(&out_of_order_buffer_arc);
        let transport_bg = Arc::clone(&transport);

        // Spawn the UDP receive/retransmit task
        tokio::spawn(async move {
            // Use the trait object for send/recv
            let mut buf = vec![0; 1024];
            loop {
                let mut transport_locked = transport_bg.lock().await;
                tokio::select! {
                    Ok((len, addr)) = transport_locked.recv_from(&mut buf) => {
                        println!("[UDP RX] Received {} bytes from {}: {:02X?}", len, addr, &buf[..len]);
                        if let Ok((header, message)) = MessageCodec::decode(&buf[..len]) {
                            println!("[UDP RX] Decoded message: {:?} (seq: {}) from {}", message, header.sequence_id, addr);
                            match &message {
                                Message::UseCircuitCodeReply(success) => {
                                    println!("[HANDSHAKE] Received UseCircuitCodeReply: success={}", success);
                                }
                                Message::AgentMovementComplete { .. } => {
                                    println!("[HANDSHAKE] Received AgentMovementComplete!");
                                }
                                _ => {}
                            }
                            match message {
                                Message::Ack { sequence_id } => {
                                    let mut unacked_messages = unacked_messages_arc_clone.lock().await;
                                    unacked_messages.remove(&sequence_id);
                                }
                                Message::KeepAlive => {
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
                                    // Only send ACK if not KeepAlive
                                    if !matches!(received_message, Message::KeepAlive) {
                                        let ack_message = Message::Ack { sequence_id: header.sequence_id };
                                        let ack_header = PacketHeader { sequence_id: 0, flags: 0 };
                                        if let Ok(encoded_ack) = MessageCodec::encode(&ack_header, &ack_message) {
                                            if encoded_ack.len() < 7 {
                                                tracing::warn!("[BUG] Would send ACK packet < 7 bytes ({} bytes): {:02X?}. Skipping.", encoded_ack.len(), encoded_ack);
                                            } else {
                                                let _ = transport_locked.send_to(&encoded_ack, &addr).await;
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
                                let message = Message::UseCircuitCode {
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
                    _ = time::sleep(Duration::from_millis(RETRANSMISSION_TIMEOUT_MS)) => {
                        let mut messages_to_retransmit = Vec::new();
                        let mut lost_messages = Vec::new();
                        let mut unacked_messages = unacked_messages_arc_clone.lock().await;
                        unacked_messages.retain(|&seq_id, (_message, sent_time, retransmission_count, target_addr, encoded_message)| {
                            if sent_time.elapsed() > Duration::from_millis(RETRANSMISSION_TIMEOUT_MS) {
                                if *retransmission_count < MAX_RETRANSMISSIONS {
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
                            tracing::warn!("Message {} lost after {} retransmissions.", seq_id, MAX_RETRANSMISSIONS);
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
            agent_state,
        })
    }

    pub async fn send_message(&mut self, message: &Message, target: &SocketAddr) -> io::Result<usize> {
        let header = PacketHeader {
            sequence_id: self.next_sequence_number,
            flags: 0, // TODO: Define flags for ACKs, etc.
        };
        self.next_sequence_number += 1;
        let encoded = MessageCodec::encode(&header, message)?;

        // Store message for retransmission
        let mut unacked_messages = self.unacked_messages.lock().await;
        unacked_messages.insert(
            header.sequence_id,
            (message.clone(), Instant::now(), 0, target.clone(), encoded.clone()),
        );

        let mut transport = self.transport.lock().await;
        transport.send_to(&encoded, target).await
    }

    pub async fn recv_message(&mut self) -> io::Result<(PacketHeader, Message, SocketAddr)> {
        self.receiver_channel.recv().await.ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Circuit receive channel closed"))
    }

    /// Main receive loop for processing incoming UDP messages and enforcing handshake state machine
    pub async fn run_receive_loop(&mut self,
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
    ) {
        loop {
            match self.recv_message().await {
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
                        flags,
                        controls,
                        camera_at,
                        camera_eye
                    ).await;
                    // ... handle other message types as needed ...
                }
                Err(e) => {
                    eprintln!("[UDP RX] Error receiving message: {}", e);
                    break;
                }
            }
        }
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
    ) {
        use tracing::{info, warn};
        match self.handshake_state {
            HandshakeState::NotStarted => {
                info!("[HANDSHAKE] Sending UseCircuitCode");
                let mut transport = self.transport.lock().await;
                let _ = transport.send_usecircuitcode_packet_lludp(circuit_code, session_id, agent_id).await;
                self.handshake_state = HandshakeState::SentUseCircuitCode;
            }
            HandshakeState::SentUseCircuitCode => {
                info!("[HANDSHAKE] Sending CompleteAgentMovement");
                let mut transport = self.transport.lock().await;
                let _ = transport.send_complete_agent_movement_packet(agent_id, session_id, circuit_code, position, look_at).await;
                self.handshake_state = HandshakeState::SentCompleteAgentMovement;
            }
            HandshakeState::SentCompleteAgentMovement => {
                warn!("advance_handshake called in SentCompleteAgentMovement; waiting for RegionHandshake (IN)");
                // Wait for RegionHandshake (IN)
                return;
            }
            HandshakeState::ReceivedRegionHandshake => {
                info!("[HANDSHAKE] Sending RegionHandshakeReply");
                let mut transport = self.transport.lock().await;
                let _ = transport.send_region_handshake_reply_packet(agent_id, session_id, flags).await;
                self.handshake_state = HandshakeState::SentRegionHandshakeReply;
            }
            HandshakeState::SentRegionHandshakeReply => {
                info!("[HANDSHAKE] Sending AgentThrottle");
                let mut transport = self.transport.lock().await;
                let _ = transport.send_agent_throttle_packet(agent_id, session_id, circuit_code, throttle).await;
                self.handshake_state = HandshakeState::SentAgentThrottle;
            }
            HandshakeState::SentAgentThrottle => {
                info!("[HANDSHAKE] Sending first AgentUpdate");
                let mut transport = self.transport.lock().await;
                let _ = transport.send_agent_update_packet(agent_id, session_id, position, camera_at, camera_eye, controls).await;
                self.handshake_state = HandshakeState::SentFirstAgentUpdate;
            }
            HandshakeState::SentFirstAgentUpdate => {
                info!("[HANDSHAKE] Handshake complete. Starting EQ polling and periodic AgentUpdate.");
                self.handshake_state = HandshakeState::HandshakeComplete;
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
                    // Periodic AgentUpdate
                    let transport = self.transport.clone();
                    let agent_id = agent_id;
                    let session_id = session_id;
                    let agent_state = self.agent_state.clone();
                    tokio::spawn(async move {
                        let interval = tokio::time::Duration::from_millis(100);
                        loop {
                            let (position, camera_at, camera_eye, controls) = {
                                let state = agent_state.lock().await;
                                (state.position, state.camera_at, state.camera_eye, state.controls)
                            };
                            let mut transport = transport.lock().await;
                            let _ = transport.send_agent_update_packet(
                                agent_id,
                                session_id,
                                position,
                                camera_at,
                                camera_eye,
                                controls
                            ).await;
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

    pub async fn disconnect_and_logout(&mut self, sim_addr: &SocketAddr) {
        // Send Logout message
        let _ = self.send_message(&Message::Logout, sim_addr).await;
        // TODO: Add any additional cleanup if needed
    }

    pub async fn handle_incoming_message(&mut self, header: PacketHeader, message: Message, addr: SocketAddr,
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
    ) {
        use tracing::info;
        match &message {
            Message::RegionHandshake { .. } => {
                info!("[HANDSHAKE] Received RegionHandshake");
                self.handshake_state = HandshakeState::ReceivedRegionHandshake;
                self.advance_handshake(agent_id, session_id, circuit_code, position, look_at, throttle, flags, controls, camera_at, camera_eye).await;
            }
            Message::KeepAlive => {
                // Per SL protocol and Hippolyzer, no response is required for KeepAlive packets.
                // Only log receipt for debugging.
                info!("[UDP] Received KeepAlive from {} (no response sent)", addr);
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
        circuit.disconnect_and_logout(&addr).await;
    }
}