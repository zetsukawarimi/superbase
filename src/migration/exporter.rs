use serde_json::Value;
use std::collections::BTreeSet;

use crate::collection::Collection;
use crate::error::Result;
use crate::migration::types::{ExportOptions, MigrationFormat};
use crate::query::planner::QueryRequest;

pub fn export_data(
    collection: &Collection,
    format: MigrationFormat,
    options: Option<ExportOptions>,
) -> Result<String> {
    let opts = options.unwrap_or_default();
    let docs = collection.query(QueryRequest::default())?;

    match format {
        MigrationFormat::Json => export_json(&docs),
        MigrationFormat::NdJson => export_ndjson(&docs),
        MigrationFormat::Mongo => export_mongo(&docs),
        MigrationFormat::Csv => export_csv(&docs),
        MigrationFormat::Mysql => export_mysql(&collection.name, &docs, &opts),
        MigrationFormat::Postgres => export_postgres(&collection.name, &docs, &opts),
        MigrationFormat::Sqlite => export_sqlite(&collection.name, &docs, &opts),
    }
}

fn export_json(docs: &[Value]) -> Result<String> {
    Ok(serde_json::to_string_pretty(docs)?)
}

fn export_ndjson(docs: &[Value]) -> Result<String> {
    let mut out = String::new();
    for doc in docs {
        out.push_str(&serde_json::to_string(doc)?);
        out.push('\n');
    }
    Ok(out)
}

fn export_mongo(docs: &[Value]) -> Result<String> {
    let mut out = String::new();
    for doc in docs {
        let mut clone = doc.clone();
        if let Value::Object(ref mut map) = clone {
            if let Some(id_str) = map.get("_id").and_then(|v| v.as_str()) {
                let mut oid_map = serde_json::Map::new();
                oid_map.insert("$oid".to_string(), Value::String(id_str.to_string()));
                map.insert("_id".to_string(), Value::Object(oid_map));
            }
        }
        out.push_str(&serde_json::to_string(&clone)?);
        out.push('\n');
    }
    Ok(out)
}

fn export_csv(docs: &[Value]) -> Result<String> {
    if docs.is_empty() {
        return Ok(String::new());
    }

    // Collect all unique top-level keys
    let mut headers = BTreeSet::new();
    for doc in docs {
        if let Some(map) = doc.as_object() {
            for key in map.keys() {
                headers.insert(key.clone());
            }
        }
    }

    let headers_vec: Vec<String> = headers.into_iter().collect();
    let mut out = String::new();

    // Write header line
    for (i, h) in headers_vec.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&escape_csv_cell(h));
    }
    out.push('\n');

    // Write data rows
    for doc in docs {
        let map = doc.as_object();
        for (i, h) in headers_vec.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if let Some(val) = map.and_then(|m| m.get(h)) {
                let cell_str = match val {
                    Value::Null => "".to_string(),
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                };
                out.push_str(&escape_csv_cell(&cell_str));
            }
        }
        out.push('\n');
    }

    Ok(out)
}

fn escape_csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

fn escape_sql_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\0', "\\0")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn export_mysql(table_name: &str, docs: &[Value], _opts: &ExportOptions) -> Result<String> {
    let mut sql = String::new();
    sql.push_str(&format!(
        "-- ========================================================\n\
         -- Superbase to MySQL / MariaDB Migration Dump\n\
         -- Source Collection: {}\n\
         -- Total Records: {}\n\
         -- ========================================================\n\n\
         CREATE TABLE IF NOT EXISTS `{}` (\n\
           `_id` VARCHAR(64) NOT NULL PRIMARY KEY,\n\
           `_version` BIGINT NOT NULL DEFAULT 1,\n\
           `_created_at` BIGINT NOT NULL,\n\
           `_updated_at` BIGINT NOT NULL,\n\
           `data` JSON NOT NULL\n\
         ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;\n\n",
        table_name,
        docs.len(),
        table_name
    ));

    if docs.is_empty() {
        return Ok(sql);
    }

    sql.push_str(&format!(
        "INSERT INTO `{}` (`_id`, `_version`, `_created_at`, `_updated_at`, `data`) VALUES\n",
        table_name
    ));

    for (i, doc) in docs.iter().enumerate() {
        let id = doc.get("_id").and_then(|v| v.as_str()).unwrap_or("");
        let ver = doc.get("_version").and_then(|v| v.as_u64()).unwrap_or(1);
        let created_at = doc.get("_created_at").and_then(|v| v.as_u64()).unwrap_or(0);
        let updated_at = doc.get("_updated_at").and_then(|v| v.as_u64()).unwrap_or(0);

        let json_str = serde_json::to_string(doc)?;
        let escaped_json = escape_sql_string(&json_str);

        sql.push_str(&format!(
            "('{}', {}, {}, {}, '{}')",
            escape_sql_string(id),
            ver,
            created_at,
            updated_at,
            escaped_json
        ));

        if i == docs.len() - 1 {
            sql.push_str(";\n");
        } else {
            sql.push_str(",\n");
        }
    }

    Ok(sql)
}

fn export_postgres(table_name: &str, docs: &[Value], _opts: &ExportOptions) -> Result<String> {
    let mut sql = String::new();
    sql.push_str(&format!(
        "-- ========================================================\n\
         -- Superbase to PostgreSQL Migration Dump (Native JSONB)\n\
         -- Source Collection: {}\n\
         -- Total Records: {}\n\
         -- ========================================================\n\n\
         CREATE TABLE IF NOT EXISTS \"{}\" (\n\
           \"_id\" TEXT NOT NULL PRIMARY KEY,\n\
           \"_version\" BIGINT NOT NULL DEFAULT 1,\n\
           \"_created_at\" BIGINT NOT NULL,\n\
           \"_updated_at\" BIGINT NOT NULL,\n\
           \"data\" JSONB NOT NULL\n\
         );\n\
         CREATE INDEX IF NOT EXISTS \"idx_{}_data\" ON \"{}\" USING GIN (\"data\");\n\n",
        table_name,
        docs.len(),
        table_name,
        table_name,
        table_name
    ));

    if docs.is_empty() {
        return Ok(sql);
    }

    sql.push_str(&format!(
        "INSERT INTO \"{}\" (\"_id\", \"_version\", \"_created_at\", \"_updated_at\", \"data\") VALUES\n",
        table_name
    ));

    for (i, doc) in docs.iter().enumerate() {
        let id = doc.get("_id").and_then(|v| v.as_str()).unwrap_or("");
        let ver = doc.get("_version").and_then(|v| v.as_u64()).unwrap_or(1);
        let created_at = doc.get("_created_at").and_then(|v| v.as_u64()).unwrap_or(0);
        let updated_at = doc.get("_updated_at").and_then(|v| v.as_u64()).unwrap_or(0);

        let json_str = serde_json::to_string(doc)?;
        let escaped_json = json_str.replace('\'', "''");

        sql.push_str(&format!(
            "('{}', {}, {}, {}, '{}'::jsonb)",
            id.replace('\'', "''"),
            ver,
            created_at,
            updated_at,
            escaped_json
        ));

        if i == docs.len() - 1 {
            sql.push_str(";\n");
        } else {
            sql.push_str(",\n");
        }
    }

    Ok(sql)
}

fn export_sqlite(table_name: &str, docs: &[Value], _opts: &ExportOptions) -> Result<String> {
    let mut sql = String::new();
    sql.push_str(&format!(
        "-- ========================================================\n\
         -- Superbase to SQLite3 Migration Dump\n\
         -- Source Collection: {}\n\
         -- Total Records: {}\n\
         -- ========================================================\n\n\
         CREATE TABLE IF NOT EXISTS \"{}\" (\n\
           \"_id\" TEXT NOT NULL PRIMARY KEY,\n\
           \"_version\" INTEGER NOT NULL DEFAULT 1,\n\
           \"_created_at\" INTEGER NOT NULL,\n\
           \"_updated_at\" INTEGER NOT NULL,\n\
           \"data\" TEXT NOT NULL\n\
         );\n\n",
        table_name,
        docs.len(),
        table_name
    ));

    if docs.is_empty() {
        return Ok(sql);
    }

    sql.push_str(&format!(
        "INSERT INTO \"{}\" (\"_id\", \"_version\", \"_created_at\", \"_updated_at\", \"data\") VALUES\n",
        table_name
    ));

    for (i, doc) in docs.iter().enumerate() {
        let id = doc.get("_id").and_then(|v| v.as_str()).unwrap_or("");
        let ver = doc.get("_version").and_then(|v| v.as_u64()).unwrap_or(1);
        let created_at = doc.get("_created_at").and_then(|v| v.as_u64()).unwrap_or(0);
        let updated_at = doc.get("_updated_at").and_then(|v| v.as_u64()).unwrap_or(0);

        let json_str = serde_json::to_string(doc)?;
        let escaped_json = json_str.replace('\'', "''");

        sql.push_str(&format!(
            "('{}', {}, {}, {}, '{}')",
            id.replace('\'', "''"),
            ver,
            created_at,
            updated_at,
            escaped_json
        ));

        if i == docs.len() - 1 {
            sql.push_str(";\n");
        } else {
            sql.push_str(",\n");
        }
    }

    Ok(sql)
}
