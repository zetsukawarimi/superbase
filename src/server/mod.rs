pub mod handlers;
pub mod routes;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::db::Database;
use crate::error::Result;
use routes::create_router;

pub async fn run_server(db: Arc<Database>, addr: SocketAddr) -> Result<()> {
    let app = create_router(db);
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("🚀 Superbase Server running on http://{}", addr);

    axum::serve(listener, app).await.map_err(|e| crate::error::SuperbaseError::Other(e.to_string()))?;
    Ok(())
}
