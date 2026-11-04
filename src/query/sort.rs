use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use crate::index::path::extract_json_path;
use crate::index::secondary::IndexValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SortDirection {
    Num(i32),
    Text(String),
}

impl SortDirection {
    pub fn is_desc(&self) -> bool {
        match self {
            SortDirection::Num(n) => *n < 0,
            SortDirection::Text(s) => s.eq_ignore_ascii_case("desc"),
        }
    }
}

pub fn sort_documents(docs: &mut [Value], sort_fields: &std::collections::HashMap<String, SortDirection>) {
    if sort_fields.is_empty() {
        return;
    }

    docs.sort_by(|a, b| {
        for (path, direction) in sort_fields {
            let val_a = extract_json_path(a, path).and_then(IndexValue::from_json);
            let val_b = extract_json_path(b, path).and_then(IndexValue::from_json);

            let ord = match (val_a, val_b) {
                (Some(va), Some(vb)) => va.cmp(&vb),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            };

            if ord != Ordering::Equal {
                return if direction.is_desc() {
                    ord.reverse()
                } else {
                    ord
                };
            }
        }
        Ordering::Equal
    });
}

pub fn apply_projection(doc: &Value, fields: &[String]) -> Value {
    if fields.is_empty() {
        return doc.clone();
    }

    let mut projected = Map::new();
    for field in fields {
        if let Some(val) = extract_json_path(doc, field) {
            // For simple top-level fields or nested
            if !field.contains('.') {
                projected.insert(field.clone(), val.clone());
            } else {
                // Nested path insertion
                let parts: Vec<&str> = field.split('.').collect();
                insert_nested(&mut projected, &parts, val.clone());
            }
        }
    }

    Value::Object(projected)
}

fn insert_nested(map: &mut Map<String, Value>, parts: &[&str], value: Value) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
        return;
    }

    let entry = map.entry(parts[0].to_string()).or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(sub_map) = entry {
        insert_nested(sub_map, &parts[1..], value);
    }
}
