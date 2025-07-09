// Second Life UDP Packet Format Demo

// Header (6 bytes):
// - Flags (1 byte): Reliability/acknowledgment flags
// - Sequence Number (4 bytes): Packet ordering
// - Extra Header Size (1 byte): Additional header data

// Message Body:
// - Message Type (1 byte): 0x12 for ObjectUpdate
// - Region Handle (8 bytes): Identifies the simulator region
// - Timestamp (4 bytes): Message timestamp
// - Object Count (1 byte): Number of objects in this update

// Per Object (variable length):
// - Local ID (4 bytes): Object identifier
// - Full ID (16 bytes): UUID of the object
// - Parent ID (4 bytes): Parent object reference
// - CRC (4 bytes): Checksum for caching
// - Material (1 byte): Surface material type
// - Click Action (1 byte): Default click behavior
// - Position (12 bytes): X, Y, Z coordinates
// - Velocity (12 bytes): Movement vector
// - Acceleration (12 bytes): Physics acceleration
// - Rotation (16 bytes): Quaternion rotation
// - Angular Velocity (12 bytes): Rotation speed
// - Scale (12 bytes): Object size
// - Texture Entry (variable): Texture mapping data
// - Additional Data (variable): Custom object data

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Second Life Protocol Message Types
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum MessageType {
    ObjectUpdate = 0x12,
    ObjectUpdateCompressed = 0x13,
    ObjectUpdateCached = 0x14,
    RegionHandshake = 0x94,
    AgentUpdate = 0x04,
    ChatFromSimulator = 0x80,
    KillObject = 0x15,
}

// Second Life UDP Packet Header Structure
#[derive(Debug)]
pub struct PacketHeader {
    pub flags: u8,           // Reliability, resend, ack flags
    pub sequence_number: u32, // Packet sequence number
    pub extra_header_size: u8, // Size of extra header data
}

// Vector3 structure as used in Second Life
#[derive(Debug, Clone, Copy)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// Quaternion for rotations
#[derive(Debug, Clone, Copy)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

// Object data structure based on Second Life's ObjectUpdate message
#[derive(Debug, Clone)]
pub struct ObjectData {
    pub local_id: u32,
    pub full_id: [u8; 16],        // UUID
    pub parent_id: u32,
    pub crc: u32,
    pub material: u8,
    pub click_action: u8,
    pub position: Vector3,
    pub velocity: Vector3,
    pub acceleration: Vector3,
    pub rotation: Quaternion,
    pub angular_velocity: Vector3,
    pub scale: Vector3,
    pub texture_entry: Vec<u8>,   // Texture data
    pub data: Vec<u8>,           // Additional object data
}

// Second Life Message Structure
#[derive(Debug)]
pub struct SLMessage {
    pub header: PacketHeader,
    pub message_type: MessageType,
    pub region_handle: u64,
    pub timestamp: u32,
    pub objects: Vec<ObjectData>,
}

// Scene object for rendering
#[derive(Debug, Clone)]
pub struct SceneObject {
    pub id: u32,
    pub position: Vector3,
    pub rotation: Quaternion,
    pub scale: Vector3,
    pub velocity: Vector3,
    pub last_update: Instant,
    pub active: bool,
}

// Simple scene manager
pub struct SceneManager {
    pub objects: HashMap<u32, SceneObject>,
    pub last_frame_time: Instant,
}

impl SceneManager {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            last_frame_time: Instant::now(),
        }
    }

    pub fn update_object(&mut self, object_data: &ObjectData) {
        let scene_object = SceneObject {
            id: object_data.local_id,
            position: object_data.position,
            rotation: object_data.rotation,
            scale: object_data.scale,
            velocity: object_data.velocity,
            last_update: Instant::now(),
            active: true,
        };
        
        self.objects.insert(object_data.local_id, scene_object);
        println!("Updated object {}: pos({:.2}, {:.2}, {:.2}), rot({:.2}, {:.2}, {:.2}, {:.2})", 
                 object_data.local_id,
                 object_data.position.x, object_data.position.y, object_data.position.z,
                 object_data.rotation.x, object_data.rotation.y, object_data.rotation.z, object_data.rotation.w);
    }

    pub fn remove_object(&mut self, local_id: u32) {
        if let Some(mut obj) = self.objects.get_mut(&local_id) {
            obj.active = false;
            println!("Removed object {}", local_id);
        }
    }

    pub fn render_frame(&mut self) {
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        println!("\n--- FRAME RENDER (dt: {:.3}s) ---", delta_time);
        
        // Update object positions based on velocity
        for (id, obj) in self.objects.iter_mut() {
            if obj.active {
                // Simple physics integration
                obj.position.x += obj.velocity.x * delta_time;
                obj.position.y += obj.velocity.y * delta_time;
                obj.position.z += obj.velocity.z * delta_time;
                
                println!("Object {}: pos({:.2}, {:.2}, {:.2}), scale({:.2}, {:.2}, {:.2})",
                         id,
                         obj.position.x, obj.position.y, obj.position.z,
                         obj.scale.x, obj.scale.y, obj.scale.z);
            }
        }
        
        // Remove inactive objects older than 5 seconds
        let cutoff_time = now - Duration::from_secs(5);
        self.objects.retain(|_, obj| obj.active || obj.last_update > cutoff_time);
        
        println!("--- END FRAME ({} objects) ---\n", self.objects.len());
    }
}

// Message serialization/deserialization helpers
impl SLMessage {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        // Header
        buffer.push(self.header.flags);
        buffer.extend_from_slice(&self.header.sequence_number.to_le_bytes());
        buffer.push(self.header.extra_header_size);
        
        // Message type
        buffer.push(self.message_type as u8);
        
        // Region handle and timestamp
        buffer.extend_from_slice(&self.region_handle.to_le_bytes());
        buffer.extend_from_slice(&self.timestamp.to_le_bytes());
        
        // Number of objects
        buffer.push(self.objects.len() as u8);
        
        // Object data
        for obj in &self.objects {
            buffer.extend_from_slice(&obj.local_id.to_le_bytes());
            buffer.extend_from_slice(&obj.full_id);
            buffer.extend_from_slice(&obj.parent_id.to_le_bytes());
            buffer.extend_from_slice(&obj.crc.to_le_bytes());
            buffer.push(obj.material);
            buffer.push(obj.click_action);
            
            // Position
            buffer.extend_from_slice(&obj.position.x.to_le_bytes());
            buffer.extend_from_slice(&obj.position.y.to_le_bytes());
            buffer.extend_from_slice(&obj.position.z.to_le_bytes());
            
            // Velocity
            buffer.extend_from_slice(&obj.velocity.x.to_le_bytes());
            buffer.extend_from_slice(&obj.velocity.y.to_le_bytes());
            buffer.extend_from_slice(&obj.velocity.z.to_le_bytes());
            
            // Acceleration
            buffer.extend_from_slice(&obj.acceleration.x.to_le_bytes());
            buffer.extend_from_slice(&obj.acceleration.y.to_le_bytes());
            buffer.extend_from_slice(&obj.acceleration.z.to_le_bytes());
            
            // Rotation
            buffer.extend_from_slice(&obj.rotation.x.to_le_bytes());
            buffer.extend_from_slice(&obj.rotation.y.to_le_bytes());
            buffer.extend_from_slice(&obj.rotation.z.to_le_bytes());
            buffer.extend_from_slice(&obj.rotation.w.to_le_bytes());
            
            // Angular velocity
            buffer.extend_from_slice(&obj.angular_velocity.x.to_le_bytes());
            buffer.extend_from_slice(&obj.angular_velocity.y.to_le_bytes());
            buffer.extend_from_slice(&obj.angular_velocity.z.to_le_bytes());
            
            // Scale
            buffer.extend_from_slice(&obj.scale.x.to_le_bytes());
            buffer.extend_from_slice(&obj.scale.y.to_le_bytes());
            buffer.extend_from_slice(&obj.scale.z.to_le_bytes());
            
            // Texture entry length and data
            buffer.extend_from_slice(&(obj.texture_entry.len() as u16).to_le_bytes());
            buffer.extend_from_slice(&obj.texture_entry);
            
            // Additional data length and data
            buffer.extend_from_slice(&(obj.data.len() as u16).to_le_bytes());
            buffer.extend_from_slice(&obj.data);
        }
        
        buffer
    }
    
    pub fn deserialize(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 14 {
            return Err("Packet too short");
        }
        
        let mut offset = 0;
        
        // Parse header
        let flags = data[offset];
        offset += 1;
        
        let sequence_number = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]);
        offset += 4;
        
        let extra_header_size = data[offset];
        offset += 1;
        
        let message_type = match data[offset] {
            0x12 => MessageType::ObjectUpdate,
            0x13 => MessageType::ObjectUpdateCompressed,
            0x14 => MessageType::ObjectUpdateCached,
            0x94 => MessageType::RegionHandshake,
            0x04 => MessageType::AgentUpdate,
            0x80 => MessageType::ChatFromSimulator,
            0x15 => MessageType::KillObject,
            _ => return Err("Unknown message type"),
        };
        offset += 1;
        
        let region_handle = u64::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]
        ]);
        offset += 8;
        
        let timestamp = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]);
        offset += 4;
        
        let num_objects = data[offset] as usize;
        offset += 1;
        
        let mut objects = Vec::new();
        
        for _ in 0..num_objects {
            if offset + 100 > data.len() {
                break; // Not enough data for complete object
            }
            
            let local_id = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]);
            offset += 4;
            
            let mut full_id = [0u8; 16];
            full_id.copy_from_slice(&data[offset..offset + 16]);
            offset += 16;
            
            let parent_id = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]);
            offset += 4;
            
            let crc = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]);
            offset += 4;
            
            let material = data[offset];
            offset += 1;
            
            let click_action = data[offset];
            offset += 1;
            
            // Parse vectors and quaternion
            let position = Vector3 {
                x: f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                y: f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]),
                z: f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]),
            };
            offset += 12;
            
            let velocity = Vector3 {
                x: f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                y: f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]),
                z: f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]),
            };
            offset += 12;
            
            let acceleration = Vector3 {
                x: f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                y: f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]),
                z: f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]),
            };
            offset += 12;
            
            let rotation = Quaternion {
                x: f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                y: f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]),
                z: f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]),
                w: f32::from_le_bytes([data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15]]),
            };
            offset += 16;
            
            let angular_velocity = Vector3 {
                x: f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                y: f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]),
                z: f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]),
            };
            offset += 12;
            
            let scale = Vector3 {
                x: f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                y: f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]),
                z: f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]),
            };
            offset += 12;
            
            // Parse texture entry
            let texture_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            
            let texture_entry = if offset + texture_len <= data.len() {
                data[offset..offset + texture_len].to_vec()
            } else {
                Vec::new()
            };
            offset += texture_len;
            
            // Parse additional data
            let data_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            
            let obj_data = if offset + data_len <= data.len() {
                data[offset..offset + data_len].to_vec()
            } else {
                Vec::new()
            };
            offset += data_len;
            
            objects.push(ObjectData {
                local_id,
                full_id,
                parent_id,
                crc,
                material,
                click_action,
                position,
                velocity,
                acceleration,
                rotation,
                angular_velocity,
                scale,
                texture_entry,
                data: obj_data,
            });
        }
        
        Ok(SLMessage {
            header: PacketHeader {
                flags,
                sequence_number,
                extra_header_size,
            },
            message_type,
            region_handle,
            timestamp,
            objects,
        })
    }
}

// Mock server that generates realistic Second Life update messages
pub struct MockSecondLifeServer {
    sequence_number: u32,
    objects: Vec<ObjectData>,
}

impl MockSecondLifeServer {
    pub fn new() -> Self {
        Self {
            sequence_number: 1,
            objects: Vec::new(),
        }
    }
    
    pub fn generate_object_update(&mut self) -> SLMessage {
        // Create some sample objects with realistic data
        if self.objects.is_empty() {
            // Add some initial objects
            for i in 0..5 {
                self.objects.push(ObjectData {
                    local_id: 1000 + i,
                    full_id: [i as u8; 16],
                    parent_id: 0,
                    crc: 0x12345678,
                    material: 3, // Stone
                    click_action: 0,
                    position: Vector3 {
                        x: (i as f32) * 10.0,
                        y: (i as f32) * 5.0,
                        z: 0.0,
                    },
                    velocity: Vector3 {
                        x: (i as f32 - 2.0) * 0.5,
                        y: 0.0,
                        z: 0.0,
                    },
                    acceleration: Vector3 { x: 0.0, y: 0.0, z: -9.81 },
                    rotation: Quaternion { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
                    angular_velocity: Vector3 { x: 0.0, y: 0.0, z: 0.1 },
                    scale: Vector3 { x: 1.0, y: 1.0, z: 1.0 },
                    texture_entry: vec![0xFF, 0xFF, 0xFF, 0xFF], // Default texture
                    data: vec![0x00, 0x01, 0x02, 0x03], // Sample data
                });
            }
        }
        
        // Update object positions and create message
        let message = SLMessage {
            header: PacketHeader {
                flags: 0x00, // No special flags
                sequence_number: self.sequence_number,
                extra_header_size: 0,
            },
            message_type: MessageType::ObjectUpdate,
            region_handle: 0x1000000010000000, // Sample region handle
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32,
            objects: self.objects.clone(),
        };
        
        self.sequence_number += 1;
        message
    }
}

// Main demo function
pub fn run_demo() {
    println!("=== Second Life Protocol Rendering Demo ===\n");
    
    let mut server = MockSecondLifeServer::new();
    let mut scene = SceneManager::new();
    
    println!("Demo simulates Second Life UDP protocol messages");
    println!("Port: 12035 (typical SL UDP port)");
    println!("Protocol: Binary UDP with custom message format\n");
    
    // Simulate receiving and processing messages
    for frame in 0..10 {
        println!("--- FRAME {} ---", frame);
        
        // Generate a server message
        let message = server.generate_object_update();
        
        // Serialize the message (this is what goes over the network)
        let serialized = message.serialize();
        println!("Serialized message size: {} bytes", serialized.len());
        
        // Show raw packet data (first 64 bytes)
        print!("Raw packet data: ");
        for (i, byte) in serialized.iter().take(64).enumerate() {
            if i > 0 && i % 16 == 0 {
                print!("\n                 ");
            }
            print!("{:02X} ", byte);
        }
        if serialized.len() > 64 {
            print!("... ({} more bytes)", serialized.len() - 64);
        }
        println!("\n");
        
        // Deserialize the message (client receives this)
        match SLMessage::deserialize(&serialized) {
            Ok(received_message) => {
                println!("Received message: {:?}", received_message.message_type);
                println!("Sequence: {}", received_message.header.sequence_number);
                println!("Objects in message: {}", received_message.objects.len());
                
                // Update scene with received objects
                for obj in &received_message.objects {
                    scene.update_object(obj);
                }
            }
            Err(e) => {
                println!("Failed to deserialize message: {}", e);
            }
        }
        
        // Render frame
        scene.render_frame();
        
        // Simulate 60 FPS
        thread::sleep(Duration::from_millis(16));
    }
    
    println!("\n=== Demo Complete ===");
    println!("In a real implementation, you would:");
    println!("1. Listen on UDP port 12035");
    println!("2. Parse incoming packets using this message format");
    println!("3. Update your 3D scene based on ObjectUpdate messages");
    println!("4. Handle other message types (chat, agent updates, etc.)");
    println!("5. Implement proper packet acknowledgment and reliability");
}

// Example usage
fn main() {
    run_demo();
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_serialization() {
        let mut server = MockSecondLifeServer::new();
        let original_message = server.generate_object_update();
        
        let serialized = original_message.serialize();
        let deserialized = SLMessage::deserialize(&serialized).unwrap();
        
        assert_eq!(original_message.header.sequence_number, deserialized.header.sequence_number);
        assert_eq!(original_message.objects.len(), deserialized.objects.len());
        
        if let (Some(orig), Some(deser)) = (original_message.objects.first(), deserialized.objects.first()) {
            assert_eq!(orig.local_id, deser.local_id);
            assert_eq!(orig.position.x, deser.position.x);
            assert_eq!(orig.position.y, deser.position.y);
            assert_eq!(orig.position.z, deser.position.z);
        }
    }
}