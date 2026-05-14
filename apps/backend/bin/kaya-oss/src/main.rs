//! kaya-oss — OSS self-hosted binary (Apache 2.0)
//!
//! HTTP server on `KAYA_PORT` (default 3001).
//! Pass `--schema` to print the OpenAPI JSON and exit (CI codegen).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::Extension,
    http::{Method, StatusCode, header},
    response::Response,
};
use rust_embed::RustEmbed;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tokio::sync::Mutex;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

use kaya_core::model_router::ModelRouter;
use kaya_server::state::StoredEdit;
use kaya_storage::{SqliteAdapter, SqliteSessionStorage};

// ── OpenAPI ───────────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(title = "Kaya Suites API", version = "0.1.0",
         description = "OSS self-hosted Kaya Suites backend"),
    paths(health)
)]
struct ApiDoc;

// ── Route: GET /health ────────────────────────────────────────────────────────

#[utoipa::path(
    get, path = "/health",
    responses((status = 200, description = "Service is healthy", body = Value,
               example = json!({"status": "ok"}))),
    tag = "ops"
)]
async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

// ── Embedded static frontend ──────────────────────────────────────────────────

#[derive(RustEmbed)]
#[folder = "frontend/"]
struct Assets;

async fn static_handler(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidates = [
        path.to_string(),
        format!("{path}.html"),
        "index.html".to_string(),
    ];

    for candidate in &candidates {
        if let Some(content) = Assets::get(candidate) {
            let mime = mime_guess::from_path(candidate)
                .first_or_octet_stream()
                .to_string();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(content.data.to_vec()))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))
        .unwrap()
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let (oa_router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .split_for_parts();

    if std::env::args().any(|a| a == "--schema") {
        println!("{}", api.to_pretty_json().expect("serialise OpenAPI schema"));
        return;
    }

    // ── Storage ──────────────────────────────────────────────────────────────
    let content_dir = std::env::var("KAYA_CONTENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("content"));
    let db_path = std::env::var("KAYA_DB_PATH")
        .unwrap_or_else(|_| "kaya.db".to_string());

    let storage: Arc<dyn kaya_core::StorageAdapter> = Arc::new(
        SqliteAdapter::new(content_dir, Path::new(&db_path))
            .await
            .expect("open storage"),
    );

    // ── LLM router (optional — chat returns 503 if unavailable) ──────────────
    let config_path = std::env::var("KAYA_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("kaya.yaml"));

    let llm: Option<Arc<ModelRouter>> = match ModelRouter::from_yaml(&config_path) {
        Ok(r) => {
            eprintln!("kaya-oss: LLM router loaded from {config_path:?}");
            Some(Arc::new(r))
        }
        Err(e) => {
            eprintln!("kaya-oss: LLM router unavailable ({e}); chat will return 503");
            None
        }
    };

    // ── Sessions DB ───────────────────────────────────────────────────────────
    let sessions_pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await
        .expect("open sessions pool");

    SqliteSessionStorage::migrate(&sessions_pool)
        .await
        .expect("sessions migration");

    let sessions: Arc<dyn kaya_core::SessionStorage> =
        Arc::new(SqliteSessionStorage::new(sessions_pool));

    // ── Pending edits ─────────────────────────────────────────────────────────
    let pending_edits: Arc<Mutex<HashMap<uuid::Uuid, StoredEdit>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // ── CORS ──────────────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(AllowHeaders::any());

    // ── Router ────────────────────────────────────────────────────────────────
    let app = oa_router
        .merge(kaya_server::router())
        .fallback(static_handler)
        .layer(Extension(storage))
        .layer(Extension(sessions))
        .layer(Extension(llm))
        .layer(Extension(pending_edits))
        .layer(cors);

    let port: u16 = std::env::var("KAYA_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));

    eprintln!("kaya-oss listening on {addr}");
    axum::serve(listener, app).await.expect("server error");
}
