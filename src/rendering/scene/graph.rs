use std::collections::HashMap;
use crate::rendering::scene::Object;

pub struct SceneGraph {
    nodes: HashMap<u32, Object>,
    // Add hierarchical relationships later
}

impl SceneGraph {
    pub fn new() -> Self {
        SceneGraph {
            nodes: HashMap::new(),
        }
    }

    pub fn add_object(&mut self, id: u32, object: Object) {
        self.nodes.insert(id, object);
    }

    pub fn remove_object(&mut self, id: u32) -> Option<Object> {
        self.nodes.remove(&id)
    }

    pub fn update_object(&mut self, id: u32, new_object: Object) -> Option<Object> {
        self.nodes.insert(id, new_object)
    }
}
