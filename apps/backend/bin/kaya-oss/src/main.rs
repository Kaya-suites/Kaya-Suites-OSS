//! kaya-oss — OSS self-hosted binary (Apache 2.0)
//!
//! Starts an axum HTTP server on port 3001 (configurable via `KAYA_PORT`).
//! Pass `--schema` to print the OpenAPI JSON to stdout and exit — used by
//! the CI codegen pipeline.

use axum::Json;
use serde_json::{json, Value};
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

// ── OpenAPI document ─────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Kaya Suites API",
        version = "0.1.0",
        description = "OSS self-hosted Kaya Suites backend"
    ),
    paths(health)
)]
struct ApiDoc;

// ── Routes ───────────────────────────────────────────────────────────────────

/// Health check — returns `{"status":"ok"}`.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy",
         body = Value,
         example = json!({"status": "ok"}))
    ),
    tag = "ops"
)]
async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .split_for_parts();

    // `--schema` mode: emit OpenAPI JSON and exit (used by CI codegen).
    if std::env::args().any(|a| a == "--schema") {
        println!("{}", api.to_pretty_json().expect("serialise OpenAPI schema"));
        return;
    }

    let port: u16 = std::env::var("KAYA_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));

    eprintln!("kaya-oss listening on {addr}");
    axum::serve(listener, router)
        .await
        .expect("server error");
}
