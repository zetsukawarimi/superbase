use serde_json::Value;

/// Extracts a value from a JSON document using dot notation
/// e.g. "user.profile.age" will traverse { "user": { "profile": { "age": 25 } } }
pub fn extract_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for part in path.split('.') {
        match current {
            Value::Object(map) => {
                if let Some(next) = map.get(part) {
                    current = next;
                } else {
                    return None;
                }
            }
            Value::Array(arr) => {
                // If part is a numeric index, access array element
                if let Ok(idx) = part.parse::<usize>() {
                    if let Some(next) = arr.get(idx) {
                        current = next;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_nested() {
        let doc = json!({
            "name": "Alice",
            "profile": {
                "age": 30,
                "address": {
                    "city": "Jakarta"
                }
            },
            "tags": ["rust", "database"]
        });

        assert_eq!(extract_json_path(&doc, "name").unwrap(), &json!("Alice"));
        assert_eq!(extract_json_path(&doc, "profile.age").unwrap(), &json!(30));
        assert_eq!(extract_json_path(&doc, "profile.address.city").unwrap(), &json!("Jakarta"));
        assert_eq!(extract_json_path(&doc, "tags.0").unwrap(), &json!("rust"));
        assert_eq!(extract_json_path(&doc, "profile.unknown"), None);
    }
}
