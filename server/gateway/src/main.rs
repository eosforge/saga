//! Saga gateway entrypoint providing core RESTful routing and health inspection endpoints.

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tracing::info;

async fn health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "gateway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn ping() -> Json<Value> {
    let sample = b"saga gateway verification";
    let digest = protocol::hash(sample);
    Json(json!({
        "message": "pong",
        "sample_hash": digest
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/health", get(health)).route("/api/ping", get(ping));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("gateway server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
