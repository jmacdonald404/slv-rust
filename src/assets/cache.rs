use std::collections::HashMap;
use std::hash::Hash;

pub struct AssetCache<K, V> {
    cache: HashMap<K, V>,
}

impl<K, V> AssetCache<K, V>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        AssetCache {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.cache.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.cache.insert(key, value);
    }
}
