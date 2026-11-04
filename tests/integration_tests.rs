use serde_json::json;
use std::collections::HashMap;
use superbase::{Database, DatabaseOptions, QueryFilter, QueryRequest, SortDirection};
use superbase::query::filter::Condition;

#[test]
fn test_crud_and_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().to_path_buf();

    // 1. Create DB and insert documents
    {
        let db = Database::open(&db_path, Some(DatabaseOptions { sync_immediate: true })).unwrap();
        let users = db.collection("users").unwrap();

        let doc1 = json!({
            "name": "Budi Santoso",
            "age": 25,
            "role": "admin",
            "skills": ["rust", "linux"]
        });
        let inserted1 = users.insert(doc1).unwrap();
        let id1 = inserted1["_id"].as_str().unwrap().to_string();

        let doc2 = json!({
            "name": "Siti Rahma",
            "age": 30,
            "role": "engineer",
            "skills": ["database", "rust"]
        });
        let inserted2 = users.insert(doc2).unwrap();
        let id2 = inserted2["_id"].as_str().unwrap().to_string();

        assert_eq!(users.count(None).unwrap(), 2);

        // Fetch
        let fetched1 = users.get(&id1).unwrap().unwrap();
        assert_eq!(fetched1["name"], "Budi Santoso");

        // Update
        let update_doc = json!({
            "name": "Budi Santoso",
            "age": 26,
            "role": "admin"
        });
        let updated = users.update(&id1, update_doc).unwrap();
        assert_eq!(updated["_version"], 2);
        assert_eq!(updated["age"], 26);

        // Delete Siti
        assert!(users.delete(&id2).unwrap());
        assert_eq!(users.count(None).unwrap(), 1);
    }

    // 2. Re-open DB (Testing Crash Recovery & Replay)
    {
        let db = Database::open(&db_path, None).unwrap();
        let users = db.collection("users").unwrap();

        assert_eq!(users.count(None).unwrap(), 1);
        let docs = users.query(QueryRequest::default()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0]["name"], "Budi Santoso");
        assert_eq!(docs[0]["age"], 26);
        assert_eq!(docs[0]["_version"], 2);
    }
}

#[test]
fn test_secondary_indexing_and_complex_queries() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::open(temp_dir.path(), None).unwrap();
    let products = db.collection("products").unwrap();

    // Create index on nested price and category
    products.create_index("details.price").unwrap();
    products.create_index("category").unwrap();

    products.insert(json!({
        "title": "Laptop Gaming",
        "category": "electronics",
        "details": { "price": 1500, "in_stock": true }
    })).unwrap();

    products.insert(json!({
        "title": "Mechanical Keyboard",
        "category": "electronics",
        "details": { "price": 120, "in_stock": true }
    })).unwrap();

    products.insert(json!({
        "title": "Coffee Maker",
        "category": "home",
        "details": { "price": 85, "in_stock": true }
    })).unwrap();

    products.insert(json!({
        "title": "Wireless Mouse",
        "category": "electronics",
        "details": { "price": 45, "in_stock": false }
    })).unwrap();

    // Query 1: Filter electronics with price >= 100
    let mut conds = HashMap::new();
    conds.insert("category".into(), Condition::Direct(json!("electronics")));

    let mut price_ops = HashMap::new();
    price_ops.insert("$gte".into(), json!(100));
    conds.insert("details.price".into(), Condition::Complex(price_ops));

    let filter = QueryFilter { conditions: conds, ..Default::default() };

    let mut sort = HashMap::new();
    sort.insert("details.price".into(), SortDirection::Text("desc".into()));

    let req = QueryRequest {
        filter: Some(filter),
        sort: Some(sort),
        ..Default::default()
    };

    let results = products.query(req).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["title"], "Laptop Gaming");
    assert_eq!(results[1]["title"], "Mechanical Keyboard");

    // Test projection
    let proj_req = QueryRequest {
        project: Some(vec!["title".into(), "details.price".into()]),
        limit: Some(1),
        ..Default::default()
    };
    let projected = products.query(proj_req).unwrap();
    assert_eq!(projected.len(), 1);
    assert!(projected[0].get("title").is_some());
    assert!(projected[0].get("category").is_none());
}

#[test]
fn test_compaction() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::open(temp_dir.path(), None).unwrap();
    let col = db.collection("logs").unwrap();

    // Insert 100 docs
    let mut ids = Vec::new();
    for i in 0..100 {
        let inserted = col.insert(json!({ "index": i, "data": "payload" })).unwrap();
        ids.push(inserted["_id"].as_str().unwrap().to_string());
    }

    // Delete first 50
    for id in &ids[0..50] {
        col.delete(id).unwrap();
    }

    // Overwrite remaining 50
    for (i, id) in ids[50..100].iter().enumerate() {
        col.update(id, json!({ "index": i + 50, "data": "updated_payload" })).unwrap();
    }

    assert_eq!(col.count(None).unwrap(), 50);

    // Trigger compaction
    col.compact().unwrap();

    assert_eq!(col.count(None).unwrap(), 50);

    // Verify data remains consistent
    for id in &ids[50..100] {
        let doc = col.get(id).unwrap().unwrap();
        assert_eq!(doc["data"], "updated_payload");
    }
}
