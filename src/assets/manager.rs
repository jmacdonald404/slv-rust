use async_trait::async_trait;
use std::path::Path;
use anyhow::Result;
use crate::assets::cache::AssetCache;
use crate::assets::{Asset, texture::TextureLoader, mesh::MeshLoader};
use std::sync::Arc;
use wgpu::{Device, Queue};

#[async_trait]
pub trait AssetLoader<A> {
    async fn load(&self, path: &Path) -> Result<A>;
}

pub struct ResourceManager {
    pub cache: AssetCache<String, Asset>,
    pub texture_loader: TextureLoader,
    pub mesh_loader: MeshLoader,
}

impl ResourceManager {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            cache: AssetCache::new(),
            texture_loader: TextureLoader::new(Arc::clone(&device), Arc::clone(&queue)),
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
        match self.cache.get(&path.to_string()) {
            Some(Asset::Texture(t)) => Some(t),
            _ => None,
        }
    }

    pub fn get_mesh(&self, path: &str) -> Option<&crate::assets::mesh::Mesh> {
        match self.cache.get(&path.to_string()) {
            Some(Asset::Mesh(m)) => Some(m),
            _ => None,
        }
    }
}
