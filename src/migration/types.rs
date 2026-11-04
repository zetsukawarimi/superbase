use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MigrationFormat {
    Json,
    NdJson,
    Csv,
    Mysql,
    Postgres,
    Sqlite,
    Mongo,
}

impl MigrationFormat {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "json" => Some(MigrationFormat::Json),
            "ndjson" | "jsonl" | "lines" => Some(MigrationFormat::NdJson),
            "csv" => Some(MigrationFormat::Csv),
            "mysql" | "mariadb" => Some(MigrationFormat::Mysql),
            "postgres" | "postgresql" | "pgsql" => Some(MigrationFormat::Postgres),
            "sqlite" | "sqlite3" => Some(MigrationFormat::Sqlite),
            "mongo" | "mongodb" => Some(MigrationFormat::Mongo),
            _ => None,
        }
    }

    pub fn default_extension(&self) -> &'static str {
        match self {
            MigrationFormat::Json => "json",
            MigrationFormat::NdJson => "ndjson",
            MigrationFormat::Csv => "csv",
            MigrationFormat::Mysql => "mysql.sql",
            MigrationFormat::Postgres => "pg.sql",
            MigrationFormat::Sqlite => "sqlite.sql",
            MigrationFormat::Mongo => "mongo.json",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub collection: String,
    pub total_processed: usize,
    pub imported: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub elapsed_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportOptions {
    /// If true, relational exports (MySQL/PG/SQLite) will unpack top-level JSON fields into native SQL columns
    pub flatten_columns: bool,
}
