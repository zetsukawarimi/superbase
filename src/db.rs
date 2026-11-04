use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::collection::{Collection, CollectionStats};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub collections_count: usize,
    pub collections: Vec<CollectionStats>,
}

#[derive(Clone)]
pub struct DatabaseOptions {
    pub sync_immediate: bool,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        Self {
            sync_immediate: false, // Default to high-throughput asynchronous WAL flush
        }
    }
}

pub struct Database {
    pub base_dir: PathBuf,
    collections: DashMap<String, Arc<Collection>>,
    options: DatabaseOptions,
}

impl Database {
    pub fn open<P: AsRef<Path>>(dir: P, options: Option<DatabaseOptions>) -> Result<Arc<Self>> {
        let base_dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&base_dir)?;

        let opts = options.unwrap_or_default();
        let db = Arc::new(Self {
            base_dir: base_dir.clone(),
            collections: DashMap::new(),
            options: opts.clone(),
        });

        // Auto-discover and open existing collections in the base directory
        for entry in fs::read_dir(&base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(col_name) = path.file_name().and_then(|s| s.to_str()) {
                    let col = Collection::open(&base_dir, col_name, opts.sync_immediate)?;
                    db.collections.insert(col_name.to_string(), Arc::new(col));
                }
            }
        }

        Ok(db)
    }

    pub fn collection(&self, name: &str) -> Result<Arc<Collection>> {
        if let Some(col) = self.collections.get(name) {
            return Ok(col.clone());
        }

        // Create if does not exist
        let col = Collection::open(&self.base_dir, name, self.options.sync_immediate)?;
        let col_arc = Arc::new(col);
        self.collections.insert(name.to_string(), col_arc.clone());
        Ok(col_arc)
    }

    pub fn drop_collection(&self, name: &str) -> Result<bool> {
        if self.collections.remove(name).is_some() {
            let col_dir = self.base_dir.join(name);
            if col_dir.exists() {
                fs::remove_dir_all(col_dir)?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn list_collections(&self) -> Vec<String> {
        self.collections.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn stats(&self) -> DatabaseStats {
        let collections: Vec<CollectionStats> = self
            .collections
            .iter()
            .map(|entry| entry.value().stats())
            .collect();

        DatabaseStats {
            collections_count: collections.len(),
            collections,
        }
    }

    pub fn flush_all(&self) -> Result<()> {
        for entry in self.collections.iter() {
            entry.value().flush()?;
        }
        Ok(())
    }
}
