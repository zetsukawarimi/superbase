use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::db::Database;
use crate::server::handlers::*;

pub fn create_router(db: Arc<Database>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/stats", get(get_database_stats))
        .route("/api/v1/collections", get(list_collections).post(create_collection))
        .route("/api/v1/collections/{collection}", delete(drop_collection))
        .route("/api/v1/collections/{collection}/stats", get(get_collection_stats))
        .route("/api/v1/collections/{collection}/compact", post(compact_collection))
        .route("/api/v1/collections/{collection}/documents", post(insert_document))
        .route(
            "/api/v1/collections/{collection}/documents/{id}",
            get(get_document).put(update_document).delete(delete_document),
        )
        .route("/api/v1/collections/{collection}/query", post(query_documents))
        .route(
            "/api/v1/collections/{collection}/indexes",
            get(list_indexes).post(create_index),
        )
        .route("/api/v1/collections/{collection}/indexes/{path}", delete(drop_index))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(db)
}
