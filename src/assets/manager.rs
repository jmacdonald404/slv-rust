
use async_trait::async_trait;
use std::path::Path;
use anyhow::Result;

#[async_trait]
pub trait AssetLoader<A> {
    async fn load(&self, path: &Path) -> Result<A>;
}
