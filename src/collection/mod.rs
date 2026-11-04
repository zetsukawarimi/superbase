pub mod document;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

use crate::error::{Result, SuperbaseError};
use crate::index::manager::IndexManager;
use crate::index::primary::PrimaryIndex;
use crate::query::filter::QueryFilter;
use crate::query::planner::{QueryPlanner, QueryRequest};
use crate::query::sort::{apply_projection, sort_documents};
use crate::wal::record::OpType;
use crate::wal::WalEngine;
use document::{prepare_insert, prepare_update};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionStats {
    pub name: String,
    pub document_count: usize,
    pub segment_count: usize,
    pub indexed_fields: Vec<String>,
}

pub struct Collection {
    pub name: String,
    wal: Arc<WalEngine>,
    primary_index: PrimaryIndex,
    index_manager: IndexManager,
    cache: DashMap<String, Arc<Value>>,
}

impl Collection {
    pub fn open<P: AsRef<Path>>(
        base_dir: P,
        name: &str,
        sync_immediate: bool,
    ) -> Result<Self> {
        let collection_dir = base_dir.as_ref().join(name);
        std::fs::create_dir_all(&collection_dir)?;

        let wal = Arc::new(WalEngine::open(&collection_dir, None, sync_immediate)?);
        let primary_index = PrimaryIndex::new();
        let index_manager = IndexManager::new();
        let cache = DashMap::new();

        let collection = Self {
            name: name.to_string(),
            wal,
            primary_index,
            index_manager,
            cache,
        };

        collection.recover()?;

        Ok(collection)
    }

    /// Replay WAL segments to rebuild primary index and cache max sequence
    fn recover(&self) -> Result<()> {
        let segment_ids = self.wal.list_segments();
        let mut max_seq = 0u64;

        for seg_id in segment_ids {
            if let Some(segment) = self.wal.get_segment(seg_id) {
                let records = segment.scan_all()?;
                for (ptr, record) in records {
                    if record.seq > max_seq {
                        max_seq = record.seq;
                    }
                    match record.op {
                        OpType::Put => {
                            self.primary_index.insert(record.key.clone(), ptr);
                            // Also optionally populate cache or secondary index if json
                            if let Ok(doc) = serde_json::from_slice::<Value>(&record.value) {
                                self.index_manager.on_insert(&record.key, &doc);
                                self.cache.insert(record.key, Arc::new(doc));
                            }
                        }
                        OpType::Delete => {
                            if let Some(_) = self.primary_index.remove(&record.key) {
                                if let Some((_, old_doc)) = self.cache.remove(&record.key) {
                                    self.index_manager.on_delete(&record.key, &old_doc);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.wal.set_seq(max_seq + 1);
        Ok(())
    }

    pub fn insert(&self, doc: Value) -> Result<Value> {
        let (id, prepared_doc) = prepare_insert(doc, None);
        self.insert_internal(id, prepared_doc)
    }

    pub fn insert_with_id(&self, id: String, doc: Value) -> Result<Value> {
        let (actual_id, prepared_doc) = prepare_insert(doc, Some(id));
        self.insert_internal(actual_id, prepared_doc)
    }

    fn insert_internal(&self, id: String, doc: Value) -> Result<Value> {
        if self.primary_index.contains_key(&id) {
            return Err(SuperbaseError::Other(format!(
                "Document with ID '{}' already exists. Use update instead.",
                id
            )));
        }

        let val_bytes = serde_json::to_vec(&doc)?;
        let ptr = self.wal.write_put(id.clone(), val_bytes)?;

        self.primary_index.insert(id.clone(), ptr);
        self.index_manager.on_insert(&id, &doc);
        let doc_arc = Arc::new(doc.clone());
        self.cache.insert(id, doc_arc);

        Ok(doc)
    }

    pub fn get(&self, id: &str) -> Result<Option<Value>> {
        // Fast path: In-memory cache
        if let Some(cached) = self.cache.get(id) {
            return Ok(Some((**cached).clone()));
        }

        // Secondary path: Read from WAL
        let ptr = match self.primary_index.get(id) {
            Some(p) => p,
            None => return Ok(None),
        };

        let record = self.wal.read_record(&ptr)?;
        let doc: Value = serde_json::from_slice(&record.value)?;
        let doc_arc = Arc::new(doc.clone());
        self.cache.insert(id.to_string(), doc_arc);

        Ok(Some(doc))
    }

    pub fn update(&self, id: &str, new_doc: Value) -> Result<Value> {
        let current_doc = self.get(id)?.ok_or_else(|| SuperbaseError::NotFound(id.to_string()))?;
        let updated_doc = prepare_update(id, new_doc, &current_doc);

        let val_bytes = serde_json::to_vec(&updated_doc)?;
        let ptr = self.wal.write_put(id.to_string(), val_bytes)?;

        self.primary_index.insert(id.to_string(), ptr);
        self.index_manager.on_update(id, &current_doc, &updated_doc);
        self.cache.insert(id.to_string(), Arc::new(updated_doc.clone()));

        Ok(updated_doc)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        if !self.primary_index.contains_key(id) {
            return Ok(false);
        }

        let old_doc = self.get(id)?;
        self.wal.write_delete(id.to_string())?;
        self.primary_index.remove(id);

        if let Some(doc) = old_doc {
            self.index_manager.on_delete(id, &doc);
        }
        self.cache.remove(id);

        Ok(true)
    }

    pub fn query(&self, req: QueryRequest) -> Result<Vec<Value>> {
        let mut matched_docs = Vec::new();

        // 1. Determine candidate IDs using Secondary Indexes if available
        let candidate_ids = if let Some(ref filter) = req.filter {
            QueryPlanner::plan_candidates(filter, &self.index_manager)
        } else {
            None
        };

        let target_ids: Vec<String> = match candidate_ids {
            Some(set) => set.into_iter().collect(),
            None => self.primary_index.keys(),
        };

        // 2. Fetch and filter documents
        for id in target_ids {
            if let Some(doc) = self.get(&id)? {
                if let Some(ref filter) = req.filter {
                    if !filter.matches(&doc) {
                        continue;
                    }
                }
                matched_docs.push(doc);
            }
        }

        // 3. Sort documents if requested
        if let Some(ref sort_fields) = req.sort {
            sort_documents(&mut matched_docs, sort_fields);
        }

        // 4. Pagination: skip & limit
        let skip = req.skip.unwrap_or(0);
        let docs_iter = matched_docs.into_iter().skip(skip);

        let limited_docs: Vec<Value> = if let Some(limit) = req.limit {
            docs_iter.take(limit).collect()
        } else {
            docs_iter.collect()
        };

        // 5. Projection: select specific fields
        let result = if let Some(ref fields) = req.project {
            limited_docs.into_iter().map(|d| apply_projection(&d, fields)).collect()
        } else {
            limited_docs
        };

        Ok(result)
    }

    pub fn count(&self, filter: Option<QueryFilter>) -> Result<usize> {
        if let Some(filter) = filter {
            let req = QueryRequest {
                filter: Some(filter),
                ..Default::default()
            };
            let docs = self.query(req)?;
            Ok(docs.len())
        } else {
            Ok(self.primary_index.len())
        }
    }

    pub fn create_index(&self, path: &str) -> Result<()> {
        self.index_manager.create_index(path)?;

        // Populate index from existing documents
        for id in self.primary_index.keys() {
            if let Some(doc) = self.get(&id)? {
                self.index_manager.on_insert(&id, &doc);
            }
        }

        Ok(())
    }

    pub fn drop_index(&self, path: &str) -> bool {
        self.index_manager.drop_index(path)
    }

    pub fn list_indexes(&self) -> Vec<String> {
        self.index_manager.list_indexes()
    }

    pub fn compact(&self) -> Result<()> {
        let live_entries = self.primary_index.all_entries();
        let new_pointers = self.wal.compact(&live_entries)?;

        for (key, ptr) in new_pointers {
            self.primary_index.insert(key, ptr);
        }

        Ok(())
    }

    pub fn stats(&self) -> CollectionStats {
        CollectionStats {
            name: self.name.clone(),
            document_count: self.primary_index.len(),
            segment_count: self.wal.list_segments().len(),
            indexed_fields: self.index_manager.list_indexes(),
        }
    }

    pub fn flush(&self) -> Result<()> {
        self.wal.flush()
    }
}
