use crate::networking::transport::UdpTransport;
use crate::networking::protocol::messages::{PacketHeader, Message};
use crate::networking::protocol::codecs::MessageCodec;
use std::net::SocketAddr;
use std::io;
use std::collections::HashMap;
use tokio::time::{self, Instant, Duration};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

const RETRANSMISSION_TIMEOUT_MS: u64 = 200;
const MAX_RETRANSMISSIONS: u32 = 5;

pub struct Circuit {
    transport: UdpTransport,
    next_sequence_number: u32,
    next_expected_sequence_number: Arc<Mutex<u32>>,
    unacked_messages: Arc<Mutex<HashMap<u32, (Message, Instant, u32, SocketAddr, Vec<u8>)>>>, // sequence_id -> (message, sent_time, retransmission_count, target_addr, encoded_message)
    receiver_channel: mpsc::Receiver<(PacketHeader, Message, SocketAddr)>, // Channel for receiving messages from the processing task
    out_of_order_buffer: Arc<Mutex<HashMap<u32, (PacketHeader, Message, SocketAddr)>>>, // sequence_id -> (header, message, sender_addr)
}

impl Circuit {
    pub async fn new(bind_addr: &str, _remote_addr: SocketAddr) -> std::io::Result<Self> {
        let transport = UdpTransport::new(bind_addr).await?;
        let (sender_channel_for_task, receiver_channel) = mpsc::channel(100);

        let unacked_messages_arc = Arc::new(Mutex::new(HashMap::<u32, (Message, Instant, u32, SocketAddr, Vec<u8>)>::new()));
        let unacked_messages_arc_clone = Arc::clone(&unacked_messages_arc);
        let bind_addr_string = bind_addr.to_string();
        let next_expected_sequence_number_arc = Arc::new(Mutex::new(1));
        let next_expected_sequence_number_arc_clone = Arc::clone(&next_expected_sequence_number_arc);
        let out_of_order_buffer_arc = Arc::new(Mutex::new(HashMap::<u32, (PacketHeader, Message, SocketAddr)>::new()));
        let out_of_order_buffer_arc_clone = Arc::clone(&out_of_order_buffer_arc);

        tokio::spawn(async move {
            let transport_task = UdpTransport::new(&bind_addr_string).await.expect("Failed to bind UDP socket in spawned task");
            let mut buf = vec![0; 1024];
            loop {
                tokio::select! {
                    Ok((len, addr)) = transport_task.recv(&mut buf) => {
                        if let Ok((header, message)) = MessageCodec::decode(&buf[..len]) {
                            // Process received message
                            match message {
                                Message::Ack { sequence_id } => {
                                    // Acquire lock, remove, then release lock immediately
                                    if let Ok(mut unacked_messages) = unacked_messages_arc_clone.lock() {
                                        unacked_messages.remove(&sequence_id);
                                    }
                                }
                                received_message => {
                                    let mut messages_to_send = Vec::new();
                                    {
                                        let mut current_expected_seq = next_expected_sequence_number_arc_clone.lock().unwrap();
                                        let mut out_of_order_buffer = out_of_order_buffer_arc_clone.lock().unwrap();

                                        if header.sequence_id == *current_expected_seq {
                                            // Process in-order packet
                                            messages_to_send.push((header.clone(), received_message.clone(), addr));
                                            *current_expected_seq += 1;

                                            // Check buffer for consecutive packets
                                            while let Some((h, m, a)) = out_of_order_buffer.remove(&*current_expected_seq) {
                                                messages_to_send.push((h, m, a));
                                                *current_expected_seq += 1;
                                            }
                                        } else if header.sequence_id > *current_expected_seq {
                                            // Store out-of-order packet
                                            out_of_order_buffer.insert(header.sequence_id, (header.clone(), received_message.clone(), addr));
                                        } else {
                                            // Duplicate or already processed, discard
                                            println!("Discarding duplicate or old packet: {:?}", header);
                                        }
                                    }

                                    // Send ACK for received message
                                    let ack_message = Message::Ack { sequence_id: header.sequence_id };
                                    let ack_header = PacketHeader { sequence_id: 0, flags: 0 }; // ACK messages don't need sequence numbers for now
                                    if let Ok(encoded_ack) = MessageCodec::encode(&ack_header, &ack_message) {
                                        // Send ACK, this is an await point
                                        let _ = transport_task.send(&encoded_ack, &addr).await;
                                    }

                                    // Send processed messages to main circuit
                                    for (h, m, a) in messages_to_send {
                                        let _ = sender_channel_for_task.send((h, m, a)).await;
                                    }
                                }
                            }
                        }
                    },
                    _ = time::sleep(Duration::from_millis(RETRANSMISSION_TIMEOUT_MS)) => {
                        let mut messages_to_retransmit = Vec::new();
                        let mut lost_messages = Vec::new();

                        if let Ok(mut unacked_messages) = unacked_messages_arc_clone.lock() {
                            unacked_messages.retain(|&seq_id, (_message, sent_time, retransmission_count, target_addr, encoded_message)| {
                                if sent_time.elapsed() > Duration::from_millis(RETRANSMISSION_TIMEOUT_MS) {
                                    if *retransmission_count < MAX_RETRANSMISSIONS {
                                        messages_to_retransmit.push((seq_id, target_addr.clone(), encoded_message.to_vec()));
                                        *sent_time = Instant::now();
                                        *retransmission_count += 1;
                                        true // Retain in map
                                    } else {
                                        lost_messages.push(seq_id);
                                        false // Remove from map
                                    }
                                } else {
                                    true // Retain in map
                                }
                            });
                        }

                        for (seq_id, target_addr, encoded_message) in messages_to_retransmit {
                            let _ = transport_task.send(&encoded_message, &target_addr).await;
                            println!("Retransmitting message {} to {}.", seq_id, target_addr);
                        }
                        for seq_id in lost_messages {
                            println!("Message {} lost after {} retransmissions.", seq_id, MAX_RETRANSMISSIONS);
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
        if let Ok(mut unacked_messages) = self.unacked_messages.lock() {
            unacked_messages.insert(
                header.sequence_id,
                (message.clone(), Instant::now(), 0, target.clone(), encoded.clone()),
            );
        }

        self.transport.send(&encoded, target).await
    }

    pub async fn recv_message(&mut self) -> io::Result<(PacketHeader, Message, SocketAddr)> {
        self.receiver_channel.recv().await.ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Circuit receive channel closed"))
    }
}