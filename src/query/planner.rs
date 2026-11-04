use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::Bound;

use crate::index::manager::IndexManager;
use crate::index::secondary::{IndexValue, SecondaryIndex};
use crate::query::filter::{Condition, QueryFilter};
use crate::query::sort::SortDirection;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryRequest {
    #[serde(default)]
    pub filter: Option<QueryFilter>,
    #[serde(default)]
    pub sort: Option<HashMap<String, SortDirection>>,
    #[serde(default)]
    pub skip: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub project: Option<Vec<String>>,
}

pub struct QueryPlanner;

impl QueryPlanner {
    /// Checks if secondary indexes can be used to narrow candidate document IDs
    pub fn plan_candidates(
        filter: &QueryFilter,
        index_manager: &IndexManager,
    ) -> Option<HashSet<String>> {
        let mut candidate_set: Option<HashSet<String>> = None;

        for (path, condition) in &filter.conditions {
            if let Some(index) = index_manager.get_index(path) {
                let matches = Self::evaluate_index_condition(&index, condition);
                if let Some(matching_ids) = matches {
                    match candidate_set {
                        Some(ref mut existing) => {
                            // Intersect sets
                            *existing = existing.intersection(&matching_ids).cloned().collect();
                        }
                        None => {
                            candidate_set = Some(matching_ids);
                        }
                    }
                }
            }
        }

        candidate_set
    }

    fn evaluate_index_condition(
        index: &SecondaryIndex,
        condition: &Condition,
    ) -> Option<HashSet<String>> {
        match condition {
            Condition::Direct(target) => {
                let val = IndexValue::from_json(target)?;
                Some(index.find_eq(&val))
            }
            Condition::Complex(ops) => {
                // Check if equality or range query
                if let Some(eq_val) = ops.get("$eq") {
                    let val = IndexValue::from_json(eq_val)?;
                    return Some(index.find_eq(&val));
                }

                if let Some(in_val) = ops.get("$in") {
                    if let Some(arr) = in_val.as_array() {
                        let vals: Vec<IndexValue> = arr.iter().filter_map(IndexValue::from_json).collect();
                        return Some(index.find_in(&vals));
                    }
                }

                // Range query bounds
                let mut lower = Bound::Unbounded;
                let mut upper = Bound::Unbounded;
                let mut has_range = false;

                if let Some(gt) = ops.get("$gt").and_then(IndexValue::from_json) {
                    lower = Bound::Excluded(gt);
                    has_range = true;
                } else if let Some(gte) = ops.get("$gte").and_then(IndexValue::from_json) {
                    lower = Bound::Included(gte);
                    has_range = true;
                }

                if let Some(lt) = ops.get("$lt").and_then(IndexValue::from_json) {
                    upper = Bound::Excluded(lt);
                    has_range = true;
                } else if let Some(lte) = ops.get("$lte").and_then(IndexValue::from_json) {
                    upper = Bound::Included(lte);
                    has_range = true;
                }

                if has_range {
                    let start_bound = match &lower {
                        Bound::Included(v) => Bound::Included(v),
                        Bound::Excluded(v) => Bound::Excluded(v),
                        Bound::Unbounded => Bound::Unbounded,
                    };
                    let end_bound = match &upper {
                        Bound::Included(v) => Bound::Included(v),
                        Bound::Excluded(v) => Bound::Excluded(v),
                        Bound::Unbounded => Bound::Unbounded,
                    };

                    return Some(index.find_range(start_bound, end_bound));
                }

                None
            }
        }
    }
}
