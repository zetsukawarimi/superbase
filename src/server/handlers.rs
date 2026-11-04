use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::db::Database;
use crate::query::planner::QueryRequest;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> (StatusCode, Json<Self>) {
        (
            StatusCode::OK,
            Json(Self {
                success: true,
                data: Some(data),
                error: None,
            }),
        )
    }

    pub fn created(data: T) -> (StatusCode, Json<Self>) {
        (
            StatusCode::CREATED,
            Json(Self {
                success: true,
                data: Some(data),
                error: None,
            }),
        )
    }

    pub fn err(status: StatusCode, message: &str) -> (StatusCode, Json<Self>) {
        (
            status,
            Json(Self {
                success: false,
                data: None,
                error: Some(message.to_string()),
            }),
        )
    }
}

pub async fn health_check() -> impl IntoResponse {
    ApiResponse::ok(json!({
        "status": "healthy",
        "engine": "Superbase",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Blazing fast JSON document database in 100% Rust"
    }))
}

pub async fn get_database_stats(
    State(db): State<Arc<Database>>,
) -> impl IntoResponse {
    ApiResponse::ok(db.stats())
}

pub async fn list_collections(
    State(db): State<Arc<Database>>,
) -> impl IntoResponse {
    ApiResponse::ok(db.list_collections())
}

#[derive(Deserialize)]
pub struct CreateCollectionPayload {
    pub name: String,
}

pub async fn create_collection(
    State(db): State<Arc<Database>>,
    Json(payload): Json<CreateCollectionPayload>,
) -> impl IntoResponse {
    match db.collection(&payload.name) {
        Ok(col) => ApiResponse::created(col.stats()),
        Err(e) => ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn drop_collection(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    match db.drop_collection(&collection) {
        Ok(true) => ApiResponse::ok(json!({ "dropped": true })),
        Ok(false) => ApiResponse::err(StatusCode::NOT_FOUND, "Collection not found"),
        Err(e) => ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn insert_document(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
    Json(doc): Json<Value>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.insert(doc) {
        Ok(inserted) => ApiResponse::created(inserted),
        Err(e) => ApiResponse::err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn get_document(
    State(db): State<Arc<Database>>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.get(&id) {
        Ok(Some(doc)) => ApiResponse::ok(doc),
        Ok(None) => ApiResponse::err(StatusCode::NOT_FOUND, &format!("Document '{}' not found", id)),
        Err(e) => ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn update_document(
    State(db): State<Arc<Database>>,
    Path((collection, id)): Path<(String, String)>,
    Json(doc): Json<Value>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.update(&id, doc) {
        Ok(updated) => ApiResponse::ok(updated),
        Err(e) => ApiResponse::err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn delete_document(
    State(db): State<Arc<Database>>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.delete(&id) {
        Ok(true) => ApiResponse::ok(json!({ "deleted": true, "id": id })),
        Ok(false) => ApiResponse::err(StatusCode::NOT_FOUND, &format!("Document '{}' not found", id)),
        Err(e) => ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn query_documents(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
    Json(query_req): Json<QueryRequest>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.query(query_req) {
        Ok(docs) => ApiResponse::ok(json!({
            "count": docs.len(),
            "documents": docs
        })),
        Err(e) => ApiResponse::err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct CreateIndexPayload {
    pub path: String,
}

pub async fn create_index(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
    Json(payload): Json<CreateIndexPayload>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.create_index(&payload.path) {
        Ok(()) => ApiResponse::created(json!({
            "message": format!("Index on '{}' created successfully", payload.path),
            "indexes": col.list_indexes()
        })),
        Err(e) => ApiResponse::err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn list_indexes(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    ApiResponse::ok(json!({ "indexes": col.list_indexes() }))
}

pub async fn drop_index(
    State(db): State<Arc<Database>>,
    Path((collection, path)): Path<(String, String)>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    if col.drop_index(&path) {
        ApiResponse::ok(json!({ "dropped": true, "path": path }))
    } else {
        ApiResponse::err(StatusCode::NOT_FOUND, "Index not found")
    }
}

pub async fn compact_collection(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match col.compact() {
        Ok(()) => ApiResponse::ok(json!({ "compacted": true })),
        Err(e) => ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_collection_stats(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    ApiResponse::ok(col.stats())
}

#[derive(Deserialize)]
pub struct ImportQuery {
    pub format: Option<String>,
}

pub async fn import_collection(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
    axum::extract::Query(query): axum::extract::Query<ImportQuery>,
    body: String,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let fmt_str = query.format.as_deref().unwrap_or("json");
    let format = crate::migration::MigrationFormat::from_str_loose(fmt_str)
        .unwrap_or(crate::migration::MigrationFormat::Json);

    match crate::migration::import_data(&col, format, &body) {
        Ok(report) => ApiResponse::ok(report),
        Err(e) => ApiResponse::err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
    pub flatten: Option<bool>,
}

pub async fn export_collection(
    State(db): State<Arc<Database>>,
    Path(collection): Path<String>,
    axum::extract::Query(query): axum::extract::Query<ExportQuery>,
) -> impl IntoResponse {
    let col = match db.collection(&collection) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                e.to_string(),
            )
                .into_response()
        }
    };

    let fmt_str = query.format.as_deref().unwrap_or("json");
    let format = crate::migration::MigrationFormat::from_str_loose(fmt_str)
        .unwrap_or(crate::migration::MigrationFormat::Json);

    let opts = crate::migration::ExportOptions {
        flatten_columns: query.flatten.unwrap_or(false),
    };

    match crate::migration::export_data(&col, format, Some(opts)) {
        Ok(exported_text) => {
            let content_type = match format {
                crate::migration::MigrationFormat::Json => "application/json",
                crate::migration::MigrationFormat::Csv => "text/csv; charset=utf-8",
                crate::migration::MigrationFormat::Mysql
                | crate::migration::MigrationFormat::Postgres
                | crate::migration::MigrationFormat::Sqlite => "application/sql",
                crate::migration::MigrationFormat::NdJson
                | crate::migration::MigrationFormat::Mongo => "application/x-ndjson",
            };

            let filename = format!("{}.{}", collection, format.default_extension());
            let disposition = format!("attachment; filename=\"{}\"", filename);

            (
                StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, content_type),
                    (axum::http::header::CONTENT_DISPOSITION, &disposition),
                ],
                exported_text,
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            e.to_string(),
        )
            .into_response(),
    }
}
