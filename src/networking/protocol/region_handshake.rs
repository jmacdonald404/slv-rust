//! Manual parser for Second Life LLUDP RegionHandshake packets.

use uuid::Uuid;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::networking::protocol::messages::RegionHandshakeData;

// Correctly parse RegionHandshake according to message_template.msg
pub fn parse_region_handshake(mut payload: &[u8]) -> Option<RegionHandshakeData> {
    let mut cursor = Cursor::new(&mut payload);

    // RegionInfo block
    let region_flags = cursor.read_u32::<LittleEndian>().ok()?;
    let sim_access = cursor.read_u8().ok()?;
    let sim_owner = read_uuid(&mut cursor).ok()?;
    let is_estate_manager = cursor.read_u8().ok()?;
    let water_height = cursor.read_f32::<LittleEndian>().ok()?;
    let billable_factor = cursor.read_f32::<LittleEndian>().ok()?;
    let cache_id = read_uuid(&mut cursor).ok()?;
    let region_id = read_uuid(&mut cursor).ok()?;

    // Terrain block (simplified for now, as the template is complex)
    let mut terrain_base = [Uuid::nil(); 4];
    for i in 0..4 {
        terrain_base[i] = read_uuid(&mut cursor).ok()?;
    }
    let mut terrain_detail = [Uuid::nil(); 4];
    for i in 0..4 {
        terrain_detail[i] = read_uuid(&mut cursor).ok()?;
    }
    let mut terrain_start_height = [0.0f32; 4];
    for i in 0..4 {
        terrain_start_height[i] = cursor.read_f32::<LittleEndian>().ok()?;
    }
    let mut terrain_height_range = [0.0f32; 4];
    for i in 0..4 {
        terrain_height_range[i] = cursor.read_f32::<LittleEndian>().ok()?;
    }
    
    // SimulatorVersion block
    let version_len = cursor.read_u8().ok()? as usize;
    let mut version_buf = vec![0u8; version_len];
    cursor.read_exact(&mut version_buf).ok()?;
    let _simulator_version = String::from_utf8_lossy(&version_buf).to_string();

    // RegionName is part of a different message (RegionInfo), so we'll use a placeholder
    let region_name = "Unknown".to_string();

    Some(RegionHandshakeData {
        region_flags,
        sim_access,
        region_name, // Placeholder
        sim_owner,
        is_estate_manager,
        water_height,
        billable_factor,
        cache_id,
        terrain_base,
        terrain_detail,
        terrain_start_height,
        terrain_height_range,
        region_id,
    })
}

fn read_uuid<R: Read>(reader: &mut R) -> std::io::Result<Uuid> {
    let mut buf = [0u8; 16];
    reader.read_exact(&mut buf)?;
    Ok(Uuid::from_bytes(buf))
} 