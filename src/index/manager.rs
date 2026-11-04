use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

use crate::error::{Result, SuperbaseError};
use crate::index::path::extract_json_path;
use crate::index::secondary::{IndexValue, SecondaryIndex};

#[derive(Clone, Default)]
pub struct IndexManager {
    indexes: Arc<DashMap<String, Arc<SecondaryIndex>>>,
}

impl IndexManager {
    pub fn new() -> Self {
        Self {
            indexes: Arc::new(DashMap::new()),
        }
    }

    pub fn create_index(&self, path: &str) -> Result<Arc<SecondaryIndex>> {
        if self.indexes.contains_key(path) {
            return Err(SuperbaseError::IndexError(format!("Index on '{}' already exists", path)));
        }
        let index = Arc::new(SecondaryIndex::new(path.to_string()));
        self.indexes.insert(path.to_string(), index.clone());
        Ok(index)
    }

    pub fn drop_index(&self, path: &str) -> bool {
        self.indexes.remove(path).is_some()
    }

    pub fn get_index(&self, path: &str) -> Option<Arc<SecondaryIndex>> {
        self.indexes.get(path).map(|r| r.value().clone())
    }

    pub fn list_indexes(&self) -> Vec<String> {
        self.indexes.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn on_insert(&self, doc_id: &str, doc: &Value) {
        for entry in self.indexes.iter() {
            let path = entry.key();
            let index = entry.value();
            if let Some(val) = extract_json_path(doc, path) {
                let index_values = IndexValue::from_json_multikey(val);
                index.insert_multi(index_values, doc_id);
            }
        }
    }

    pub fn on_update(&self, doc_id: &str, old_doc: &Value, new_doc: &Value) {
        for entry in self.indexes.iter() {
            let path = entry.key();
            let index = entry.value();

            if let Some(old_val) = extract_json_path(old_doc, path) {
                let old_index_values = IndexValue::from_json_multikey(old_val);
                index.remove_multi(&old_index_values, doc_id);
            }

            if let Some(new_val) = extract_json_path(new_doc, path) {
                let new_index_values = IndexValue::from_json_multikey(new_val);
                index.insert_multi(new_index_values, doc_id);
            }
        }
    }

    pub fn on_delete(&self, doc_id: &str, doc: &Value) {
        for entry in self.indexes.iter() {
            let path = entry.key();
            let index = entry.value();
            if let Some(val) = extract_json_path(doc, path) {
                let index_values = IndexValue::from_json_multikey(val);
                index.remove_multi(&index_values, doc_id);
            }
        }
    }

    pub fn clear(&self) {
        self.indexes.clear();
    }
}
