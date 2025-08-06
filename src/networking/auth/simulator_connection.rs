use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::net::UdpSocket;
use tracing::{info, debug, warn, error};
use uuid::Uuid;

use super::types::LoginResponse;
use crate::networking::packets::generated::{UseCircuitCode, CompleteAgentMovement, RegionHandshakeReply};
use crate::networking::packets::Packet;
use crate::networking::serialization::PacketSerializer;

/// Handles the post-login UDP connection establishment with Second Life simulator
pub struct SimulatorConnection {
    socket: Arc<UdpSocket>,
    simulator_addr: SocketAddr,
    login_response: LoginResponse,
    packet_serializer: std::sync::Mutex<PacketSerializer>,
}

impl SimulatorConnection {
    /// Create a new simulator connection
    pub async fn new(login_response: LoginResponse) -> Result<Self> {
        let simulator_addr = login_response.simulator_address()
            .map_err(|e| anyhow::anyhow!("Invalid simulator address from login response: {}", e))?;
        
        // Bind to a local UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .context("Failed to bind UDP socket")?;
        
        info!("🔌 Created UDP socket for simulator connection to {}", simulator_addr);
        
        Ok(Self {
            socket: Arc::new(socket),
            simulator_addr,
            login_response,
            packet_serializer: std::sync::Mutex::new(PacketSerializer::new()),
        })
    }

    /// Perform the complete post-login UDP handshake sequence
    pub async fn establish_connection(&self) -> Result<()> {
        info!("🤝 Starting post-login UDP handshake sequence with simulator {}", self.simulator_addr);
        
        // Step 1: Send UseCircuitCode
        self.send_use_circuit_code().await
            .context("Failed to send UseCircuitCode")?;
        
        // Step 2: Wait for and handle RegionHandshake (incoming from simulator)
        self.wait_for_region_handshake().await
            .context("Failed to receive RegionHandshake")?;
        
        // Step 3: Send CompleteAgentMovement
        self.send_complete_agent_movement().await
            .context("Failed to send CompleteAgentMovement")?;
        
        info!("✅ Successfully established UDP connection to simulator");
        Ok(())
    }

    /// Send UseCircuitCode packet to simulator
    async fn send_use_circuit_code(&self) -> Result<()> {
        let packet = UseCircuitCode {
            code: self.login_response.circuit_code,
            session_id: self.login_response.session_id,
            id: self.login_response.agent_id,
        };

        info!("📤 Sending UseCircuitCode packet");
        debug!("  Circuit Code: {}", self.login_response.circuit_code);
        debug!("  Session ID: {}", self.login_response.session_id);
        debug!("  Agent ID: {}", self.login_response.agent_id);
        
        self.send_packet(&packet).await
            .context("Failed to send UseCircuitCode packet")?;
        
        info!("✅ UseCircuitCode sent successfully");
        Ok(())
    }

    /// Wait for RegionHandshake from simulator and send reply
    async fn wait_for_region_handshake(&self) -> Result<()> {
        info!("⏳ Waiting for RegionHandshake from simulator...");
        
        // Set up a buffer for receiving packets
        let mut buffer = vec![0u8; 4096];
        
        // Wait for RegionHandshake with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.socket.recv_from(&mut buffer)
        ).await;

        match result {
            Ok(Ok((len, from_addr))) => {
                if from_addr != self.simulator_addr {
                    warn!("⚠️ Received packet from unexpected address: {} (expected: {})", from_addr, self.simulator_addr);
                }
                
                info!("📥 Received packet from simulator ({} bytes)", len);
                
                // TODO: Parse the packet to verify it's RegionHandshake
                // For now, we'll assume any packet is RegionHandshake and send reply
                
                self.send_region_handshake_reply().await
                    .context("Failed to send RegionHandshakeReply")?;
                
                Ok(())
            }
            Ok(Err(e)) => {
                error!("❌ UDP receive error: {}", e);
                Err(e.into())
            }
            Err(_) => {
                error!("❌ Timeout waiting for RegionHandshake from simulator");
                anyhow::bail!("Timeout waiting for RegionHandshake")
            }
        }
    }

    /// Send RegionHandshakeReply packet
    async fn send_region_handshake_reply(&self) -> Result<()> {
        let packet = RegionHandshakeReply {
            agent_id: self.login_response.agent_id,
            session_id: self.login_response.session_id,
            flags: 0, // Standard flags for region handshake reply
        };

        info!("📤 Sending RegionHandshakeReply packet");
        
        self.send_packet(&packet).await
            .context("Failed to send RegionHandshakeReply packet")?;
        
        info!("✅ RegionHandshakeReply sent successfully");
        Ok(())
    }

    /// Send CompleteAgentMovement packet
    async fn send_complete_agent_movement(&self) -> Result<()> {
        let packet = CompleteAgentMovement {
            agent_id: self.login_response.agent_id,
            session_id: self.login_response.session_id,
            circuit_code: self.login_response.circuit_code,
        };

        info!("📤 Sending CompleteAgentMovement packet");
        debug!("  Agent ID: {}", self.login_response.agent_id);
        debug!("  Session ID: {}", self.login_response.session_id);
        debug!("  Circuit Code: {}", self.login_response.circuit_code);
        
        self.send_packet(&packet).await
            .context("Failed to send CompleteAgentMovement packet")?;
        
        info!("✅ CompleteAgentMovement sent successfully");
        info!("🎉 Agent movement to simulator completed!");
        Ok(())
    }

    /// Generic method to serialize and send any packet
    async fn send_packet<P>(&self, packet: &P) -> Result<()> 
    where 
        P: Packet + std::fmt::Debug,
    {
        // Serialize packet using PacketSerializer
        let mut serializer = self.packet_serializer.lock().unwrap();
        let (serialized, sequence) = serializer.serialize(packet, P::RELIABLE)
            .map_err(|e| anyhow::anyhow!("Failed to serialize packet: {}", e))?;
        
        debug!("📤 Sending {} bytes to {} (sequence: {})", serialized.len(), self.simulator_addr, sequence);
        debug!("  Packet data: {:?}", packet);
        
        self.socket.send_to(&serialized, self.simulator_addr).await
            .context("Failed to send UDP packet")?;
        
        debug!("✅ Packet sent successfully");
        Ok(())
    }

    /// Get the simulator address
    pub fn simulator_address(&self) -> SocketAddr {
        self.simulator_addr
    }

    /// Get the login response data
    pub fn login_response(&self) -> &LoginResponse {
        &self.login_response
    }
}

/// Complete authentication flow that includes both HTTP login and UDP simulator connection
pub async fn complete_authentication(
    http_login_url: &str,
    username: &str, 
    password: &str
) -> Result<SimulatorConnection> {
    complete_authentication_with_proxy(http_login_url, username, password, None).await
}

/// Complete authentication flow with optional proxy support
pub async fn complete_authentication_with_proxy(
    http_login_url: &str,
    username: &str, 
    password: &str,
    proxy_config: Option<(&str, u16)>
) -> Result<SimulatorConnection> {
    info!("🚀 Starting complete Second Life authentication flow");
    
    // Step 1: HTTP XML-RPC Login
    info!("📡 Step 1: Performing HTTP XML-RPC login...");
    let xml_client = match proxy_config {
        Some((host, port)) => {
            info!("🔧 Using HTTP proxy: {}:{}", host, port);
            crate::networking::auth::xmlrpc::XmlRpcClient::new_with_proxy(host, port, false)?
        }
        None => {
            crate::networking::auth::xmlrpc::XmlRpcClient::new()
        }
    };
    let login_params = crate::networking::auth::xmlrpc::LoginParameters::new(username, "Resident", password);
    
    let login_response = xml_client.login_to_simulator(http_login_url, login_params).await
        .context("HTTP login failed")?;
    
    if !login_response.is_successful() {
        error!("❌ Login failed: {}", login_response.error_message().unwrap_or("Unknown error"));
        anyhow::bail!("Login failed: {}", login_response.error_message().unwrap_or("Unknown error"));
    }
    
    info!("✅ HTTP login successful for {}", login_response.full_name());
    info!("🎯 Simulator: {}:{}", login_response.simulator_ip, login_response.simulator_port);
    
    // Step 2: UDP Simulator Connection
    info!("🔌 Step 2: Establishing UDP connection to simulator...");
    let simulator_connection = SimulatorConnection::new(login_response).await
        .context("Failed to create simulator connection")?;
    
    simulator_connection.establish_connection().await
        .context("Failed to establish simulator connection")?;
    
    info!("🎉 Complete authentication flow successful!");
    Ok(simulator_connection)
}