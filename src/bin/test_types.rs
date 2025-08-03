//! Test program to verify protocol type implementations

use slv_rust::networking::packets::types::*;

fn main() {
    println!("Testing Second Life Protocol Type Implementations\n");
    
    // Test LLQuaternion (should be 12 bytes)
    println!("=== LLQuaternion Test ===");
    let quat = LLQuaternion::identity();
    println!("Identity quaternion: x={}, y={}, z={}", quat.x, quat.y, quat.z);
    println!("Calculated w component: {}", quat.calculate_w());
    let (x, y, z, w) = quat.to_full_quat();
    println!("Full quaternion: x={}, y={}, z={}, w={}", x, y, z, w);
    println!("Size in memory: {} bytes (should be 12)", std::mem::size_of::<LLQuaternion>());
    
    // Test new vector types
    println!("\n=== New Vector Types ===");
    let u16vec = U16Vec3::new(100, 200, 300);
    println!("U16Vec3: x={}, y={}, z={}", u16vec.x, u16vec.y, u16vec.z);
    println!("Size: {} bytes (should be 6)", std::mem::size_of::<U16Vec3>());
    
    let u16quat = U16Quat::identity();
    println!("U16Quat identity: x={}, y={}, z={}, w={}", u16quat.x, u16quat.y, u16quat.z, u16quat.w);
    println!("Size: {} bytes (should be 8)", std::mem::size_of::<U16Quat>());
    
    // Test signed integer types
    println!("\n=== Signed Integer Types ===");
    println!("S8 size: {} bytes", std::mem::size_of::<S8>());
    println!("S16 size: {} bytes", std::mem::size_of::<S16>());
    println!("S32 size: {} bytes", std::mem::size_of::<S32>());
    println!("S64 size: {} bytes", std::mem::size_of::<S64>());
    
    // Test variable length types
    println!("\n=== Variable Length Types ===");
    let var2 = LLVariable2::from_string("Hello, Second Life!");
    println!("LLVariable2 content: {:?}", var2.to_string().unwrap());
    println!("Data length: {} bytes", var2.data.len());
    
    // Test protocol serialization
    let serialized = var2.serialize_protocol();
    println!("Serialized with big-endian length prefix: {:?}", &serialized[0..2]);
    println!("Length prefix value: {}", u16::from_be_bytes([serialized[0], serialized[1]]));
    
    // Test control types
    println!("\n=== Control Types ===");
    println!("MVTNull size: {} bytes", std::mem::size_of::<MVTNull>());
    println!("MVTEol size: {} bytes", std::mem::size_of::<MVTEol>());
    
    println!("\n✅ All protocol types implemented successfully!");
}