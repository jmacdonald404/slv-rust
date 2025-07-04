// TODO: Implement basic avatar representation (structs, fields)
// TODO: Implement appearance loading (from asset system)
// TODO: Support mesh and texture assignment for avatars
// TODO: Add animation stubs (skeleton, pose, update)
// TODO: Integrate with scene graph and rendering

#[derive(Debug, Clone)]
pub struct Avatar {
    pub id: u32,
    pub name: String,
    pub mesh_id: Option<String>,
    pub texture_id: Option<String>,
    pub pose: AvatarPose,
}

#[derive(Debug, Clone, Default)]
pub struct AvatarPose {
    // For now, just a stub. In the future, add skeleton/joint transforms.
    pub frame: u32,
}

impl Avatar {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            mesh_id: None,
            texture_id: None,
            pose: AvatarPose::default(),
        }
    }

    pub fn assign_mesh(&mut self, mesh_id: impl Into<String>) {
        self.mesh_id = Some(mesh_id.into());
    }

    pub fn assign_texture(&mut self, texture_id: impl Into<String>) {
        self.texture_id = Some(texture_id.into());
    }

    pub fn update_animation(&mut self) {
        // For now, just increment a frame counter as a stub
        self.pose.frame = self.pose.frame.wrapping_add(1);
    }

    pub fn load_appearance(&mut self, mesh_id: impl Into<String>, texture_id: impl Into<String>) {
        self.assign_mesh(mesh_id);
        self.assign_texture(texture_id);
    }
}
