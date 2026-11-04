use serde_json::{Map, Value};
use std::time::Instant;

use crate::collection::Collection;
use crate::error::{Result, SuperbaseError};
use crate::migration::types::{ImportReport, MigrationFormat};

pub fn import_data(
    collection: &Collection,
    format: MigrationFormat,
    content: &str,
) -> Result<ImportReport> {
    let start_time = Instant::now();
    let mut imported = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    let docs = match format {
        MigrationFormat::Json => parse_json(content)?,
        MigrationFormat::NdJson => parse_ndjson(content)?,
        MigrationFormat::Mongo => parse_mongo(content)?,
        MigrationFormat::Csv => parse_csv(content)?,
        MigrationFormat::Mysql | MigrationFormat::Postgres | MigrationFormat::Sqlite => {
            parse_sql(content)?
        }
    };

    let total_processed = docs.len();

    for doc in docs {
        match collection.insert(doc) {
            Ok(_) => {
                imported += 1;
            }
            Err(e) => {
                failed += 1;
                if errors.len() < 10 {
                    errors.push(e.to_string());
                }
            }
        }
    }

    let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    Ok(ImportReport {
        collection: collection.name.clone(),
        total_processed,
        imported,
        failed,
        errors,
        elapsed_ms,
    })
}

/// Parse standard JSON (single object or array of objects)
pub fn parse_json(content: &str) -> Result<Vec<Value>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parsed: Value = serde_json::from_str(trimmed)?;
    match parsed {
        Value::Array(arr) => Ok(arr),
        Value::Object(_) => Ok(vec![parsed]),
        _ => Err(SuperbaseError::InvalidQuery(
            "Expected JSON array or object for import".into(),
        )),
    }
}

/// Parse Newline Delimited JSON (NDJSON / JSONL)
pub fn parse_ndjson(content: &str) -> Result<Vec<Value>> {
    let mut docs = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(val) => docs.push(val),
            Err(e) => {
                return Err(SuperbaseError::InvalidQuery(format!(
                    "Line {}: invalid JSON: {}",
                    line_num + 1,
                    e
                )))
            }
        }
    }
    Ok(docs)
}

/// Parse MongoDB mongoexport JSON (handles $oid, $date, etc.)
pub fn parse_mongo(content: &str) -> Result<Vec<Value>> {
    let raw_docs = if content.trim_start().starts_with('[') {
        parse_json(content)?
    } else {
        parse_ndjson(content)?
    };

    let mut transformed = Vec::with_capacity(raw_docs.len());
    for doc in raw_docs {
        transformed.push(normalize_mongo_doc(doc));
    }
    Ok(transformed)
}

fn normalize_mongo_doc(val: Value) -> Value {
    match val {
        Value::Object(mut map) => {
            // Convert { "_id": { "$oid": "xyz" } } -> { "_id": "xyz" }
            if let Some(id_val) = map.get("_id") {
                if let Some(oid) = id_val.get("$oid").and_then(|v| v.as_str()) {
                    map.insert("_id".to_string(), Value::String(oid.to_string()));
                }
            }

            // Normalize nested fields
            let mut normalized_map = Map::new();
            for (k, v) in map {
                if let Value::Object(ref sub) = v {
                    if let Some(date_val) = sub.get("$date") {
                        normalized_map.insert(k, date_val.clone());
                        continue;
                    }
                }
                normalized_map.insert(k, normalize_mongo_doc(v));
            }
            Value::Object(normalized_map)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(normalize_mongo_doc).collect()),
        other => other,
    }
}

/// Parse CSV content into structured JSON documents
pub fn parse_csv(content: &str) -> Result<Vec<Value>> {
    let mut lines = content.lines();
    let header_line = match lines.next() {
        Some(h) if !h.trim().is_empty() => h,
        _ => return Ok(Vec::new()),
    };

    let headers: Vec<String> = parse_csv_line(header_line);
    if headers.is_empty() {
        return Ok(Vec::new());
    }

    let mut docs = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let cells = parse_csv_line(trimmed);
        let mut map = Map::new();

        for (i, header) in headers.iter().enumerate() {
            let val_str = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let parsed_val = parse_inferred_value(val_str);
            map.insert(header.clone(), parsed_val);
        }

        docs.push(Value::Object(map));
    }

    Ok(docs)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    // Escaped quote: "" -> "
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }
    fields.push(current.trim().to_string());
    fields
}

fn parse_inferred_value(s: &str) -> Value {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if let Ok(b) = trimmed.parse::<bool>() {
        return Value::Bool(b);
    }
    if let Ok(i) = trimmed.parse::<i64>() {
        return Value::from(i);
    }
    if let Ok(f) = trimmed.parse::<f64>() {
        return Value::from(f);
    }
    // Check if JSON object or array
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        if let Ok(json_val) = serde_json::from_str::<Value>(trimmed) {
            return json_val;
        }
    }

    Value::String(trimmed.to_string())
}

/// Parse SQL INSERT dump (MySQL, PostgreSQL, SQLite)
pub fn parse_sql(sql_content: &str) -> Result<Vec<Value>> {
    let mut docs = Vec::new();
    let upper = sql_content.to_uppercase();
    let mut cursor = 0;

    while let Some(pos) = upper[cursor..].find("INSERT INTO") {
        let abs_pos = cursor + pos;
        // Find semicolon ending the statement
        let end_stmt = match sql_content[abs_pos..].find(';') {
            Some(p) => abs_pos + p,
            None => sql_content.len(),
        };

        let stmt = &sql_content[abs_pos..end_stmt];
        cursor = end_stmt + 1;

        // Parse INSERT INTO table_name (cols...) VALUES (vals...), (...);
        let upper_stmt = stmt.to_uppercase();
        let values_idx = match upper_stmt.find("VALUES") {
            Some(i) => i,
            None => continue,
        };

        let before_values = &stmt[..values_idx];
        let after_values = &stmt[values_idx + 6..];

        // Extract column names if specified: (col1, col2, ...)
        let mut columns = Vec::new();
        if let Some(open_p) = before_values.find('(') {
            if let Some(close_p) = before_values.rfind(')') {
                let cols_str = &before_values[open_p + 1..close_p];
                for col in cols_str.split(',') {
                    let cleaned = col.trim().trim_matches(|c| c == '`' || c == '"' || c == '\'');
                    columns.push(cleaned.to_string());
                }
            }
        }

        // Parse tuples in VALUES (...), (...)
        let tuples = parse_sql_values_tuples(after_values);
        for tuple in tuples {
            let mut map = Map::new();
            for (idx, val_str) in tuple.iter().enumerate() {
                let col_name = if idx < columns.len() {
                    columns[idx].clone()
                } else {
                    format!("col_{}", idx)
                };

                // Check if this column is a "data" JSON column
                if col_name.eq_ignore_ascii_case("data") {
                    if let Ok(Value::Object(data_obj)) = serde_json::from_str::<Value>(val_str) {
                        for (k, v) in data_obj {
                            map.insert(k, v);
                        }
                        continue;
                    }
                }

                map.insert(col_name, parse_inferred_value(val_str));
            }
            if !map.is_empty() {
                docs.push(Value::Object(map));
            }
        }
    }

    Ok(docs)
}

fn parse_sql_values_tuples(s: &str) -> Vec<Vec<String>> {
    let mut tuples = Vec::new();
    let mut in_quote = false;
    let mut quote_char = ' ';
    let mut in_tuple = false;
    let mut current_field = String::new();
    let mut current_tuple = Vec::new();
    let mut escape = false;

    for c in s.chars() {
        if escape {
            current_field.push(c);
            escape = false;
            continue;
        }

        if c == '\\' {
            escape = true;
            continue;
        }

        if in_quote {
            if c == quote_char {
                in_quote = false;
            } else {
                current_field.push(c);
            }
            continue;
        }

        match c {
            '\'' | '"' => {
                in_quote = true;
                quote_char = c;
            }
            '(' if !in_tuple => {
                in_tuple = true;
                current_tuple = Vec::new();
                current_field = String::new();
            }
            ')' if in_tuple => {
                current_tuple.push(current_field.trim().to_string());
                tuples.push(current_tuple);
                current_tuple = Vec::new();
                current_field = String::new();
                in_tuple = false;
            }
            ',' if in_tuple => {
                current_tuple.push(current_field.trim().to_string());
                current_field = String::new();
            }
            _ if in_tuple => {
                current_field.push(c);
            }
            _ => {}
        }
    }

    tuples
}
