use dashmap::DashMap;
use std::sync::Arc;
use crate::wal::segment::RecordPointer;

#[derive(Clone, Default)]
pub struct PrimaryIndex {
    map: Arc<DashMap<String, RecordPointer>>,
}

impl PrimaryIndex {
    pub fn new() -> Self {
        Self {
            map: Arc::new(DashMap::new()),
        }
    }

    #[inline]
    pub fn insert(&self, key: String, ptr: RecordPointer) -> Option<RecordPointer> {
        self.map.insert(key, ptr)
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<RecordPointer> {
        self.map.get(key).map(|v| *v)
    }

    #[inline]
    pub fn remove(&self, key: &str) -> Option<RecordPointer> {
        self.map.remove(key).map(|(_, v)| v)
    }

    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn keys(&self) -> Vec<String> {
        self.map.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn all_entries(&self) -> Vec<(String, RecordPointer)> {
        self.map.iter().map(|entry| (entry.key().clone(), *entry.value())).collect()
    }

    pub fn clear(&self) {
        self.map.clear();
    }
}
