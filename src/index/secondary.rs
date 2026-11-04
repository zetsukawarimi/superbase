use parking_lot::RwLock;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::ops::Bound;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum IndexValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl Eq for IndexValue {}

impl PartialOrd for IndexValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (IndexValue::Null, IndexValue::Null) => Ordering::Equal,
            (IndexValue::Null, _) => Ordering::Less,
            (_, IndexValue::Null) => Ordering::Greater,

            (IndexValue::Bool(a), IndexValue::Bool(b)) => a.cmp(b),
            (IndexValue::Bool(_), _) => Ordering::Less,
            (_, IndexValue::Bool(_)) => Ordering::Greater,

            (IndexValue::Int(a), IndexValue::Int(b)) => a.cmp(b),
            (IndexValue::Float(a), IndexValue::Float(b)) => a.total_cmp(b),
            (IndexValue::Int(a), IndexValue::Float(b)) => (*a as f64).total_cmp(b),
            (IndexValue::Float(a), IndexValue::Int(b)) => a.total_cmp(&(*b as f64)),

            (IndexValue::Int(_), IndexValue::String(_)) => Ordering::Less,
            (IndexValue::Float(_), IndexValue::String(_)) => Ordering::Less,
            (IndexValue::String(_), IndexValue::Int(_)) => Ordering::Greater,
            (IndexValue::String(_), IndexValue::Float(_)) => Ordering::Greater,

            (IndexValue::String(a), IndexValue::String(b)) => a.cmp(b),
        }
    }
}

impl IndexValue {
    pub fn from_json(val: &Value) -> Option<Self> {
        match val {
            Value::Null => Some(IndexValue::Null),
            Value::Bool(b) => Some(IndexValue::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(IndexValue::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Some(IndexValue::Float(f))
                } else {
                    None
                }
            }
            Value::String(s) => Some(IndexValue::String(s.clone())),
            _ => None,
        }
    }

    /// Convert JSON value to a vector of IndexValues (handles arrays for multikey indexing)
    pub fn from_json_multikey(val: &Value) -> Vec<Self> {
        match val {
            Value::Array(arr) => arr.iter().filter_map(Self::from_json).collect(),
            other => Self::from_json(other).into_iter().collect(),
        }
    }
}

#[derive(Clone)]
pub struct SecondaryIndex {
    pub path: String,
    tree: Arc<RwLock<BTreeMap<IndexValue, HashSet<String>>>>,
}

impl SecondaryIndex {
    pub fn new(path: String) -> Self {
        Self {
            path,
            tree: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub fn insert(&self, val: IndexValue, doc_id: String) {
        let mut tree = self.tree.write();
        tree.entry(val).or_default().insert(doc_id);
    }

    pub fn insert_multi(&self, vals: Vec<IndexValue>, doc_id: &str) {
        if vals.is_empty() {
            return;
        }
        let mut tree = self.tree.write();
        for val in vals {
            tree.entry(val).or_default().insert(doc_id.to_string());
        }
    }

    pub fn remove(&self, val: &IndexValue, doc_id: &str) {
        let mut tree = self.tree.write();
        if let Some(set) = tree.get_mut(val) {
            set.remove(doc_id);
            if set.is_empty() {
                tree.remove(val);
            }
        }
    }

    pub fn remove_multi(&self, vals: &[IndexValue], doc_id: &str) {
        let mut tree = self.tree.write();
        for val in vals {
            if let Some(set) = tree.get_mut(val) {
                set.remove(doc_id);
                if set.is_empty() {
                    tree.remove(val);
                }
            }
        }
    }

    pub fn find_eq(&self, val: &IndexValue) -> HashSet<String> {
        let tree = self.tree.read();
        tree.get(val).cloned().unwrap_or_default()
    }

    pub fn find_range(&self, start: Bound<&IndexValue>, end: Bound<&IndexValue>) -> HashSet<String> {
        let tree = self.tree.read();
        let mut results = HashSet::new();
        for (_, doc_ids) in tree.range((start, end)) {
            for id in doc_ids {
                results.insert(id.clone());
            }
        }
        results
    }

    pub fn find_in(&self, vals: &[IndexValue]) -> HashSet<String> {
        let tree = self.tree.read();
        let mut results = HashSet::new();
        for val in vals {
            if let Some(set) = tree.get(val) {
                for id in set {
                    results.insert(id.clone());
                }
            }
        }
        results
    }

    pub fn entry_count(&self) -> usize {
        self.tree.read().len()
    }
}
