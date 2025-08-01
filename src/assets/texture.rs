use wgpu::{Device, Queue, TextureFormat};
use image::GenericImageView;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tracing::{info, error};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum TextureError {
    #[error("Failed to load image: {0}")]
    LoadFailed(#[from] image::ImageError),
    #[error("Texture not found")]
    NotFound,
}

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Clone for Texture {
    fn clone(&self) -> Self {
        // WGPU resources can't be directly cloned, but we can share them through Arc
        // For now, we'll panic if someone tries to clone a texture
        // In a real implementation, we'd want to manage this differently
        panic!("Texture cloning not implemented - consider using Arc<Texture> instead")
    }
}

impl Texture {
    pub fn from_bytes(device: &Device, queue: &Queue, bytes: &[u8], label: &str) -> Result<Self, TextureError> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(device: &Device, queue: &Queue, img: &image::DynamicImage, label: Option<&str>) -> Result<Self, TextureError> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(
            &wgpu::TextureDescriptor {
                label,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }
        );

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }
        );

        Ok(Self { texture, view, sampler })
    }
}

pub struct TextureLoader {
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl TextureLoader {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self { device, queue }
    }
}

#[async_trait]
impl super::manager::AssetLoader<Texture> for TextureLoader {
    async fn load(&self, path: &Path) -> anyhow::Result<Texture> {
        match image::open(path) {
            Ok(img) => {
                info!("Loaded texture: {:?}", path);
                let texture = Texture::from_image(&self.device, &self.queue, &img, Some(path.to_str().unwrap_or("unnamed_texture")))?;
                Ok(texture)
            },
            Err(e) => {
                error!("Failed to load texture: {:?}, error: {}", path, e);
                Err(anyhow::anyhow!(TextureError::LoadFailed(e)))
            }
        }
    }
}
