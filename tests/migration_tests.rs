use serde_json::json;
use superbase::migration::{export_data, import_data, MigrationFormat};
use superbase::Database;

#[test]
fn test_mysql_bidirectional_migration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::open(temp_dir.path(), None).unwrap();
    let src = db.collection("mysql_source").unwrap();

    // Insert documents
    src.insert(json!({
        "username": "ahmad_dev",
        "email": "ahmad@example.com",
        "score": 98.5,
        "is_active": true,
        "profile": { "city": "Jakarta", "age": 27 }
    })).unwrap();

    src.insert(json!({
        "username": "siti_qa",
        "email": "siti@example.com",
        "score": 92.0,
        "is_active": false,
        "profile": { "city": "Surabaya", "age": 25 }
    })).unwrap();

    // 1. Export from Superbase to MySQL SQL dump
    let mysql_sql = export_data(&src, MigrationFormat::Mysql, None).unwrap();
    assert!(mysql_sql.contains("CREATE TABLE IF NOT EXISTS `mysql_source`"));
    assert!(mysql_sql.contains("INSERT INTO `mysql_source`"));
    assert!(mysql_sql.contains("ahmad_dev"));
    assert!(mysql_sql.contains("siti_qa"));

    // 2. Import MySQL SQL dump into another Superbase collection
    let dest = db.collection("mysql_target").unwrap();
    let report = import_data(&dest, MigrationFormat::Mysql, &mysql_sql).unwrap();

    assert_eq!(report.imported, 2);
    assert_eq!(report.failed, 0);

    let docs = dest.query(Default::default()).unwrap();
    assert_eq!(docs.len(), 2);
    assert!(docs.iter().any(|d| d["username"] == "ahmad_dev" && d["profile"]["city"] == "Jakarta"));
}

#[test]
fn test_postgres_bidirectional_migration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::open(temp_dir.path(), None).unwrap();
    let src = db.collection("pg_source").unwrap();

    src.insert(json!({ "product": "Keyboard", "price": 120, "tags": ["hardware"] })).unwrap();
    src.insert(json!({ "product": "Mouse", "price": 45, "tags": ["wireless"] })).unwrap();

    // Export to Postgres JSONB SQL
    let pg_sql = export_data(&src, MigrationFormat::Postgres, None).unwrap();
    assert!(pg_sql.contains("JSONB"));
    assert!(pg_sql.contains("CREATE INDEX IF NOT EXISTS"));

    // Import into target
    let dest = db.collection("pg_target").unwrap();
    let report = import_data(&dest, MigrationFormat::Postgres, &pg_sql).unwrap();
    assert_eq!(report.imported, 2);
}

#[test]
fn test_csv_bidirectional_migration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::open(temp_dir.path(), None).unwrap();
    let src = db.collection("csv_source").unwrap();

    src.insert(json!({ "name": "Budi", "age": 30, "city": "Bandung" })).unwrap();
    src.insert(json!({ "name": "Dewi", "age": 26, "city": "Medan" })).unwrap();

    // Export to CSV
    let csv_content = export_data(&src, MigrationFormat::Csv, None).unwrap();
    assert!(csv_content.contains("name") && csv_content.contains("age"));
    assert!(csv_content.contains("Budi") && csv_content.contains("Dewi"));

    // Import CSV back
    let dest = db.collection("csv_target").unwrap();
    let report = import_data(&dest, MigrationFormat::Csv, &csv_content).unwrap();
    assert_eq!(report.imported, 2);
    assert_eq!(report.failed, 0);
}

#[test]
fn test_mongo_ndjson_bidirectional_migration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::open(temp_dir.path(), None).unwrap();
    let src = db.collection("mongo_source").unwrap();

    src.insert(json!({ "sensor": "temp_1", "reading": 24.5 })).unwrap();
    src.insert(json!({ "sensor": "humidity_1", "reading": 60.0 })).unwrap();

    // Export to Mongo NDJSON
    let mongo_data = export_data(&src, MigrationFormat::Mongo, None).unwrap();
    assert!(mongo_data.contains("$oid"));

    // Import Mongo format back
    let dest = db.collection("mongo_target").unwrap();
    let report = import_data(&dest, MigrationFormat::Mongo, &mongo_data).unwrap();
    assert_eq!(report.imported, 2);
}
