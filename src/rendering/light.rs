use cgmath::{Point3, Vector3};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte alignment, we need to add some padding.
    pub _padding: u32,
    pub color: [f32; 3],
    pub _padding2: u32,
}

#[derive(Clone)]
pub struct Light {
    pub position: Point3<f32>,
    pub color: Vector3<f32>,
}

impl Light {
    pub fn to_uniform(&self) -> LightUniform {
        LightUniform {
            position: self.position.into(),
            _padding: 0,
            color: self.color.into(),
            _padding2: 0,
        }
    }
}