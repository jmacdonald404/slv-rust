pub mod manager;
pub mod cache;
pub mod mesh;
pub mod texture;
pub mod material;
pub mod shader;

pub enum Asset {
    Texture(texture::Texture),
    Mesh(mesh::Mesh),
    Material(material::Material),
    Shader(shader::Shader),
}
