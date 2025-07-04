use wgpu::{Device, Queue, TextureFormat};
use image::GenericImageView;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tracing::{info, error};

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn from_bytes(device: &Device, queue: &Queue, bytes: &[u8], label: &str) -> Result<Self, image::ImageError> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(device: &Device, queue: &Queue, img: &image::DynamicImage, label: Option<&str>) -> Result<Self, image::ImageError> {
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

pub struct TextureLoader<'a> {
    device: &'a Device,
    queue: &'a Queue,
}

impl<'a> TextureLoader<'a> {
    pub fn new(device: &'a Device, queue: &'a Queue) -> Self {
        Self { device, queue }
    }
}

#[async_trait]
impl<'a> super::manager::AssetLoader<Texture> for TextureLoader<'a> {
    async fn load(&self, path: &Path) -> Result<Texture> {
        match image::open(path) {
            Ok(img) => {
                info!("Loaded texture: {:?}", path);
                let texture = Texture::from_image(self.device, self.queue, &img, Some(path.to_str().unwrap_or("unnamed_texture")))?;
                Ok(texture)
            },
            Err(e) => {
                error!("Failed to load texture: {:?}, error: {}", path, e);
                return Err(TextureError::LoadFailed);
            }
        }
    }
}
