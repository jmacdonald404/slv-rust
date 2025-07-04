use wgpu::{Buffer, BufferUsages};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tracing::{info, error};

pub struct Mesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub num_indices: u32,
}

pub struct MeshLoader<'a> {
    device: &'a wgpu::Device,
}

impl<'a> MeshLoader<'a> {
    pub fn new(device: &'a wgpu::Device) -> Self {
        Self { device }
    }
}

#[async_trait]
impl<'a> super::manager::AssetLoader<Mesh> for MeshLoader<'a> {
    async fn load(&self, path: &Path) -> Result<Mesh> {
        // For now, we'll just create a dummy mesh.
        // In a real scenario, this would parse a mesh file (e.g., .obj, .gltf)
        // and create the appropriate buffers.

        info!("Loading mesh: {:?}", path);

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

        const VERTICES: &[Vertex] = &[
            Vertex { position: [-0.0868241, 0.49240386, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.4131759, 0.00759614] },
            Vertex { position: [-0.49513406, 0.06958647, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.0048659444, 0.43041354] },
            Vertex { position: [-0.42913406, -0.49240386, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.070865944, 0.99240386] },
            Vertex { position: [0.49513406, 0.06958647, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.99513406, 0.43041354] },
            Vertex { position: [0.0868241, -0.49240386, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.5868241, 0.99240386] },
            Vertex { position: [0.42913406, 0.49240386, 0.0], normal: [0.0, 0.0, 1.0], tex_coords: [0.92913406, 0.00759614] },
        ];

        const INDICES: &[u16] = &[
            0, 1, 4,
            0, 4, 5,
            1, 2, 4,
            2, 3, 4,
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
            vertex_buffer,
            index_buffer,
            num_indices,
        })
    }
}
