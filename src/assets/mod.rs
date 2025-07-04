pub mod manager;
pub mod cache;
pub mod mesh;
pub mod texture;

pub enum Asset {
    Texture(texture::Texture),
    // Add other asset types here
}
