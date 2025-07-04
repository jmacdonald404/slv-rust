pub mod graph;
pub mod culling;

pub struct Object {
    pub id: u32,
    pub position: cgmath::Vector3<f32>,
    pub rotation: cgmath::Quaternion<f32>,
    pub scale: cgmath::Vector3<f32>,
    pub mesh_id: String,
    pub material_id: String,
}
