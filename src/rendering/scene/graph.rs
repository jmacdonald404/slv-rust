use std::collections::HashMap;
use crate::rendering::scene::Object;
use crate::assets::mesh::Mesh;
use crate::assets::material::Material;

pub struct SceneGraph {
    pub nodes: HashMap<u32, Object>,
    // Add hierarchical relationships later
}

impl SceneGraph {
    pub fn new() -> Self {
        SceneGraph {
            nodes: HashMap::new(),
        }
    }

    pub fn add_object(&mut self, object: Object) {
        // TODO: Add object to scene graph
        self.nodes.insert(object.id, object);
    }

    pub fn remove_object(&mut self, id: u32) -> Option<Object> {
        // TODO: Remove object from scene graph
        self.nodes.remove(&id)
    }

    pub fn update_object(&mut self, id: u32, new_object: Object) -> Option<Object> {
        // TODO: Update object in scene graph
        self.nodes.insert(id, new_object)
    }

    pub fn render(&self) {
        // TODO: Integrate with rendering engine for draw calls
        // For each object, look up mesh/material by id and issue draw
        for (_id, object) in &self.nodes {
            // TODO: Fetch mesh/material from asset system using object.mesh_id/material_id
            // TODO: Set transform and issue draw call
        }
        // Stub for now
        unimplemented!()
    }
}

// TODO: Support rendering of multiple scene objects
// TODO: Implement object transforms (position, rotation, scale)
// TODO: Support per-object material and mesh assignment
// TODO: Add methods for adding, removing, updating objects
