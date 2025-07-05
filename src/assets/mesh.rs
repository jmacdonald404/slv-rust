use wgpu::{Buffer, BufferUsages, util::DeviceExt};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, error};

pub struct Mesh {
    pub vertex_buffer: Arc<Buffer>,
    pub index_buffer: Arc<Buffer>,
    pub num_indices: u32,
}

impl Clone for Mesh {
    fn clone(&self) -> Self {
        Self {
            vertex_buffer: Arc::clone(&self.vertex_buffer),
            index_buffer: Arc::clone(&self.index_buffer),
            num_indices: self.num_indices,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct MeshLoader {
    device: Arc<wgpu::Device>,
}

impl MeshLoader {
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        Self { device }
    }
}

#[async_trait]
impl super::manager::AssetLoader<Mesh> for MeshLoader {
    async fn load(&self, path: &Path) -> Result<Mesh> {
        // For now, we'll just create a dummy mesh.
        // In a real scenario, this would parse a mesh file (e.g., .obj, .gltf)
        // and create the appropriate buffers.

        info!("Loading mesh: {:?}", path);

        const VERTICES: &[Vertex] = &[
            Vertex { position: [-0.5, -0.5, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [0.5, -0.5, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [1.0, 0.0] },
            Vertex { position: [0.0, 0.5, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.5, 1.0] },
        ];

        const INDICES: &[u16] = &[
            0, 1, 2,
        ];

        let vertex_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", path)),
                contents: bytemuck::cast_slice(VERTICES),
                usage: BufferUsages::VERTEX,
            }
        );
        let index_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", path)),
                contents: bytemuck::cast_slice(INDICES),
                usage: BufferUsages::INDEX,
            }
        );
        let num_indices = INDICES.len() as u32;

        Ok(Mesh {
            vertex_buffer: Arc::new(vertex_buffer),
            index_buffer: Arc::new(index_buffer),
            num_indices,
        })
    }
}
