use super::{Result, Sentinel2ArrayError};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Metadata {
    description: String,
    hashmap: HashMap<String, String>,
}

impl Metadata {
    pub fn new(description: String) -> Self {
        Self {
            description,
            hashmap: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.hashmap.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Result<&String> {
        self.hashmap
            .get(key)
            .ok_or(Sentinel2ArrayError::MetadataKeyNotFound {
                object_desc: self.description.clone(),
                key: key.into(),
            })
    }
}
