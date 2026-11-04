use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::index::path::extract_json_path;
use crate::index::secondary::IndexValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    Direct(Value),
    Complex(OperatorMap),
}

pub type OperatorMap = std::collections::HashMap<String, Value>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryFilter {
    #[serde(default)]
    pub conditions: std::collections::HashMap<String, Condition>,
    #[serde(rename = "$or", default)]
    pub or_branches: Option<Vec<QueryFilter>>,
    #[serde(rename = "$and", default)]
    pub and_branches: Option<Vec<QueryFilter>>,
}

impl QueryFilter {
    pub fn matches(&self, doc: &Value) -> bool {
        // Top-level field conditions (AND)
        for (path, condition) in &self.conditions {
            if path.starts_with('$') {
                continue; // Handled separately or custom
            }

            let field_val = extract_json_path(doc, path);

            if !eval_condition(field_val, condition) {
                return false;
            }
        }

        // Handle $and branches
        if let Some(and_list) = &self.and_branches {
            for subfilter in and_list {
                if !subfilter.matches(doc) {
                    return false;
                }
            }
        }

        // Handle $or branches
        if let Some(or_list) = &self.or_branches {
            if !or_list.is_empty() {
                let mut matched_any = false;
                for subfilter in or_list {
                    if subfilter.matches(doc) {
                        matched_any = true;
                        break;
                    }
                }
                if !matched_any {
                    return false;
                }
            }
        }

        true
    }
}

pub fn eval_condition(field_val: Option<&Value>, cond: &Condition) -> bool {
    match cond {
        Condition::Direct(target) => match field_val {
            Some(actual) => actual == target,
            None => target.is_null(),
        },
        Condition::Complex(ops) => {
            for (op, target) in ops {
                if !eval_operator(field_val, op, target) {
                    return false;
                }
            }
            true
        }
    }
}

fn eval_operator(field_val: Option<&Value>, op: &str, target: &Value) -> bool {
    match op {
        "$eq" => match field_val {
            Some(v) => v == target,
            None => target.is_null(),
        },
        "$ne" => match field_val {
            Some(v) => v != target,
            None => !target.is_null(),
        },
        "$exists" => {
            let should_exist = target.as_bool().unwrap_or(true);
            field_val.is_some() == should_exist
        }
        "$gt" | "$gte" | "$lt" | "$lte" => {
            let actual_idx = match field_val.and_then(IndexValue::from_json) {
                Some(idx) => idx,
                None => return false,
            };
            let target_idx = match IndexValue::from_json(target) {
                Some(idx) => idx,
                None => return false,
            };

            match op {
                "$gt" => actual_idx > target_idx,
                "$gte" => actual_idx >= target_idx,
                "$lt" => actual_idx < target_idx,
                "$lte" => actual_idx <= target_idx,
                _ => false,
            }
        }
        "$in" => {
            if let Some(arr) = target.as_array() {
                match field_val {
                    Some(actual) => arr.iter().any(|item| item == actual),
                    None => arr.iter().any(|item| item.is_null()),
                }
            } else {
                false
            }
        }
        "$nin" => {
            if let Some(arr) = target.as_array() {
                match field_val {
                    Some(actual) => !arr.iter().any(|item| item == actual),
                    None => !arr.iter().any(|item| item.is_null()),
                }
            } else {
                true
            }
        }
        "$contains" => match (field_val, target) {
            (Some(Value::Array(arr)), elem) => arr.contains(elem),
            (Some(Value::String(s)), Value::String(sub)) => s.contains(sub),
            _ => false,
        },
        "$startsWith" => match (field_val, target) {
            (Some(Value::String(s)), Value::String(prefix)) => s.starts_with(prefix),
            _ => false,
        },
        "$endsWith" => match (field_val, target) {
            (Some(Value::String(s)), Value::String(suffix)) => s.ends_with(suffix),
            _ => false,
        },
        "$regex" => match (field_val, target) {
            (Some(Value::String(s)), Value::String(pattern)) => {
                if let Ok(re) = Regex::new(pattern) {
                    re.is_match(s)
                } else {
                    false
                }
            }
            _ => false,
        },
        _ => false, // Unknown operator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_filter_evaluation() {
        let doc = json!({
            "name": "Budi Santoso",
            "age": 28,
            "role": "admin",
            "skills": ["rust", "database", "backend"],
            "address": { "city": "Jakarta" }
        });

        // Test direct match
        let mut conds = std::collections::HashMap::new();
        conds.insert("role".into(), Condition::Direct(json!("admin")));
        let filter = QueryFilter { conditions: conds, ..Default::default() };
        assert!(filter.matches(&doc));

        // Test range operators
        let mut ops = OperatorMap::new();
        ops.insert("$gte".into(), json!(20));
        ops.insert("$lt".into(), json!(30));
        let mut conds2 = std::collections::HashMap::new();
        conds2.insert("age".into(), Condition::Complex(ops));
        let filter2 = QueryFilter { conditions: conds2, ..Default::default() };
        assert!(filter2.matches(&doc));

        // Test $contains on array
        let mut ops3 = OperatorMap::new();
        ops3.insert("$contains".into(), json!("rust"));
        let mut conds3 = std::collections::HashMap::new();
        conds3.insert("skills".into(), Condition::Complex(ops3));
        let filter3 = QueryFilter { conditions: conds3, ..Default::default() };
        assert!(filter3.matches(&doc));
    }
}
