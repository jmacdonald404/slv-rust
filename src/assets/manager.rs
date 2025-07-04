use async_trait::async_trait;
use std::path::Path;
use anyhow::Result;
use crate::assets::cache::AssetCache;
use crate::assets::{Asset, texture::TextureLoader, mesh::MeshLoader};
use wgpu::{Device, Queue};

#[async_trait]
pub trait AssetLoader<A> {
    async fn load(&self, path: &Path) -> Result<A>;
}

pub struct ResourceManager<'a> {
    pub cache: AssetCache<String, Asset>,
    pub texture_loader: TextureLoader<'a>,
    pub mesh_loader: MeshLoader<'a>,
}

impl<'a> ResourceManager<'a> {
    pub fn new(device: &'a Device, queue: &'a Queue) -> Self {
        Self {
            cache: AssetCache::new(),
            texture_loader: TextureLoader::new(device, queue),
            mesh_loader: MeshLoader::new(device),
        }
    }

    pub async fn load_texture(&mut self, path: &Path) -> anyhow::Result<()> {
        let texture = self.texture_loader.load(path).await?;
        self.cache.insert(path.to_str().unwrap().to_string(), Asset::Texture(texture));
        Ok(())
    }

    pub async fn load_mesh(&mut self, path: &Path) -> anyhow::Result<()> {
        let mesh = self.mesh_loader.load(path).await?;
        self.cache.insert(path.to_str().unwrap().to_string(), Asset::Mesh(mesh));
        Ok(())
    }

    pub fn get_texture(&self, path: &str) -> Option<&crate::assets::texture::Texture> {
        match self.cache.get(path) {
            Some(Asset::Texture(t)) => Some(t),
            _ => None,
        }
    }

    pub fn get_mesh(&self, path: &str) -> Option<&crate::assets::mesh::Mesh> {
        match self.cache.get(path) {
            Some(Asset::Mesh(m)) => Some(m),
            _ => None,
        }
    }
}
