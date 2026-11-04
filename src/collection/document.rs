use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

pub const FIELD_ID: &str = "_id";
pub const FIELD_VERSION: &str = "_version";
pub const FIELD_CREATED_AT: &str = "_created_at";
pub const FIELD_UPDATED_AT: &str = "_updated_at";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Prepares a document for insertion, generating _id, _version, _created_at, _updated_at
pub fn prepare_insert(mut doc: Value, custom_id: Option<String>) -> (String, Value) {
    let now = now_millis();
    let id = custom_id.unwrap_or_else(|| {
        if let Some(id_str) = doc.get(FIELD_ID).and_then(|v| v.as_str()) {
            id_str.to_string()
        } else {
            Uuid::now_v7().to_string()
        }
    });

    if let Value::Object(ref mut map) = doc {
        map.insert(FIELD_ID.to_string(), Value::String(id.clone()));
        map.insert(FIELD_VERSION.to_string(), Value::from(1u64));
        map.insert(FIELD_CREATED_AT.to_string(), Value::from(now));
        map.insert(FIELD_UPDATED_AT.to_string(), Value::from(now));
    } else {
        let mut map = Map::new();
        map.insert(FIELD_ID.to_string(), Value::String(id.clone()));
        map.insert(FIELD_VERSION.to_string(), Value::from(1u64));
        map.insert(FIELD_CREATED_AT.to_string(), Value::from(now));
        map.insert(FIELD_UPDATED_AT.to_string(), Value::from(now));
        map.insert("data".to_string(), doc);
        doc = Value::Object(map);
    }

    (id, doc)
}

/// Prepares a document for update, incrementing _version and setting _updated_at
pub fn prepare_update(id: &str, mut new_doc: Value, current_doc: &Value) -> Value {
    let now = now_millis();
    let prev_version = current_doc.get(FIELD_VERSION).and_then(|v| v.as_u64()).unwrap_or(1);
    let created_at = current_doc.get(FIELD_CREATED_AT).and_then(|v| v.as_u64()).unwrap_or(now);

    if let Value::Object(ref mut map) = new_doc {
        map.insert(FIELD_ID.to_string(), Value::String(id.to_string()));
        map.insert(FIELD_VERSION.to_string(), Value::from(prev_version + 1));
        map.insert(FIELD_CREATED_AT.to_string(), Value::from(created_at));
        map.insert(FIELD_UPDATED_AT.to_string(), Value::from(now));
    } else {
        let mut map = Map::new();
        map.insert(FIELD_ID.to_string(), Value::String(id.to_string()));
        map.insert(FIELD_VERSION.to_string(), Value::from(prev_version + 1));
        map.insert(FIELD_CREATED_AT.to_string(), Value::from(created_at));
        map.insert(FIELD_UPDATED_AT.to_string(), Value::from(now));
        map.insert("data".to_string(), new_doc);
        new_doc = Value::Object(map);
    }

    new_doc
}
