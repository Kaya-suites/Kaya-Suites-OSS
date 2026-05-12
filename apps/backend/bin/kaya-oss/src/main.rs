//! kaya-oss — OSS self-hosted binary (Apache 2.0)
//!
//! HTTP server on `KAYA_PORT` (default 3001).
//! Pass `--schema` to print the OpenAPI JSON and exit (CI codegen).

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use chrono::Utc;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool, sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions}};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use kaya_core::{
    ParagraphChange, ProposedEdit, ProposedEditKind, StorageAdapter,
    agent::{AgentContext, AgentEvent, AgentLoop, InvocationLog},
    agent::tools::default_tools,
    auth::UserSession,
    edit::commit_edit,
    model_router::ModelRouter,
};
use kaya_storage::SqliteAdapter;

// ── Domain types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    id: Uuid,
    title: String,
    created_at: i64,
    updated_at: i64,
    message_count: u32,
}

/// An edit waiting for user approval, stored in memory between the SSE stream
/// and the approve endpoint.
struct StoredEdit {
    edit: ProposedEdit,
    doc_title: String,
    first_paragraph_id: String,
    original_paragraph: String,
    proposed_paragraph: String,
}

// ── App state ─────────────────────────────────────────────────────────────────

struct AppState {
    storage: Arc<dyn StorageAdapter>,
    /// `None` when API keys are not configured; chat returns 503.
    router: Option<Arc<ModelRouter>>,
    sessions_pool: SqlitePool,
    pending_edits: Mutex<HashMap<Uuid, StoredEdit>>,
}

type S = Arc<AppState>;

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

// ── Route: GET /sessions ──────────────────────────────────────────────────────

async fn list_sessions(State(state): State<S>) -> Result<Json<Vec<Session>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, title, created_at, updated_at, message_count
         FROM sessions ORDER BY updated_at DESC",
    )
    .fetch_all(&state.sessions_pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let sessions = rows
        .into_iter()
        .map(|row| {
            Ok(Session {
                id: Uuid::parse_str(row.try_get::<&str, _>("id").map_err(|e| ApiError::internal(e.to_string()))?)
                    .map_err(|e| ApiError::internal(e.to_string()))?,
                title: row.try_get("title").map_err(|e| ApiError::internal(e.to_string()))?,
                created_at: row.try_get("created_at").map_err(|e| ApiError::internal(e.to_string()))?,
                updated_at: row.try_get("updated_at").map_err(|e| ApiError::internal(e.to_string()))?,
                message_count: row.try_get::<i64, _>("message_count").map_err(|e| ApiError::internal(e.to_string()))? as u32,
            })
        })
        .collect::<Result<Vec<Session>, ApiError>>()?;

    Ok(Json(sessions))
}

// ── Route: POST /sessions ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateSessionBody {
    title: Option<String>,
}

async fn create_session(
    State(state): State<S>,
    Json(body): Json<CreateSessionBody>,
) -> Result<(StatusCode, Json<Session>), ApiError> {
    let id = Uuid::new_v4();
    let now = Utc::now().timestamp_millis();
    let title = body.title.unwrap_or_else(|| "New conversation".to_string());

    sqlx::query(
        "INSERT INTO sessions (id, title, created_at, updated_at, message_count)
         VALUES (?, ?, ?, ?, 0)",
    )
    .bind(id.to_string())
    .bind(&title)
    .bind(now)
    .bind(now)
    .execute(&state.sessions_pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(Session { id, title, created_at: now, updated_at: now, message_count: 0 })))
}

// ── Route: GET /documents ─────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentSummary {
    id: Uuid,
    title: String,
    tags: Vec<String>,
    last_reviewed: Option<String>,
}

async fn list_documents(State(state): State<S>) -> Result<Json<Vec<DocumentSummary>>, ApiError> {
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        docs.into_iter()
            .map(|d| DocumentSummary {
                id: d.id,
                title: d.title,
                tags: d.tags,
                last_reviewed: d.last_reviewed.map(|dt| dt.to_string()),
            })
            .collect(),
    ))
}

// ── Route: GET /documents/:id ─────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentResponse {
    id: Uuid,
    title: String,
    body: String,
    tags: Vec<String>,
    last_reviewed: Option<String>,
}

async fn get_document(
    State(state): State<S>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<DocumentResponse>, ApiError> {
    let doc = state
        .storage
        .get_document(id)
        .await
        .map_err(|_| ApiError::not_found(format!("document {id}")))?;

    Ok(Json(DocumentResponse {
        id: doc.id,
        title: doc.title,
        body: doc.body,
        tags: doc.tags,
        last_reviewed: doc.last_reviewed.map(|dt| dt.to_string()),
    }))
}

// ── Route: GET /documents/:id/export.pdf ─────────────────────────────────────

async fn export_document_pdf(
    State(state): State<S>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, ApiError> {
    let doc = state
        .storage
        .get_document(id)
        .await
        .map_err(|_| ApiError::not_found(format!("document {id}")))?;

    let pdf = minimal_pdf(&doc.title, &doc.body);
    let filename = sanitize_filename(&doc.title) + ".pdf";

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(pdf))
        .unwrap())
}

// ── Route: POST /sessions/:id/chat  (SSE) ────────────────────────────────────

#[derive(Deserialize)]
struct ChatBody {
    message: String,
}

async fn chat_stream(
    State(state): State<S>,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(body): Json<ChatBody>,
) -> Response {
    let Some(router) = state.router.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "LLM provider not configured — set ANTHROPIC_API_KEY and OPENAI_API_KEY"})),
        )
            .into_response();
    };

    // Update session message count and timestamp
    let now = Utc::now().timestamp_millis();
    let _ = sqlx::query(
        "UPDATE sessions SET message_count = message_count + 1, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(session_id.to_string())
    .execute(&state.sessions_pool)
    .await;

    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let message = body.message;

    tokio::spawn(async move {
        run_agent_stream(state, router, session_id, message, tx).await;
    });

    let stream = ReceiverStream::new(rx).map(Ok::<_, Infallible>);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(stream))
        .unwrap()
}

async fn run_agent_stream(
    state: S,
    router: Arc<ModelRouter>,
    _session_id: Uuid,
    message: String,
    tx: tokio::sync::mpsc::Sender<Bytes>,
) {
    let session = UserSession { user_id: Uuid::nil() };
    let ctx = Arc::new(AgentContext {
        storage: state.storage.clone(),
        router,
        session,
    });
    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());
    let mut events = agent.run(message, ctx, log);

    // Cache doc titles seen in tool results to annotate citations cheaply.
    let mut doc_title_cache: HashMap<Uuid, String> = HashMap::new();

    macro_rules! send {
        ($data:expr) => {{
            let line = format!("data: {}\n\n", $data);
            if tx.send(Bytes::from(line)).await.is_err() {
                return; // client disconnected
            }
        }};
    }

    while let Some(result) = events.next().await {
        match result {
            Err(e) => {
                let evt = json!({"type": "Error", "message": e.to_string()});
                send!(evt);
                break;
            }

            Ok(AgentEvent::ToolResult { name, output, .. }) => {
                // Cache titles so citation lookup doesn't need a storage round-trip.
                match name.as_str() {
                    "search_documents" => {
                        if let Some(arr) = output.get("documents").and_then(|v| v.as_array()) {
                            for item in arr {
                                if let (Some(id_str), Some(title)) =
                                    (item["id"].as_str(), item["title"].as_str())
                                {
                                    if let Ok(id) = Uuid::parse_str(id_str) {
                                        doc_title_cache.insert(id, title.to_string());
                                    }
                                }
                            }
                        }
                    }
                    "read_document" => {
                        if let (Some(id_str), Some(title)) =
                            (output["id"].as_str(), output["title"].as_str())
                        {
                            if let Ok(id) = Uuid::parse_str(id_str) {
                                doc_title_cache.insert(id, title.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }

            Ok(AgentEvent::ProposedEditEmitted { edit }) => {
                if let Some(sse_data) = build_edit_sse(&state, &edit).await {
                    send!(sse_data);
                }
            }

            Ok(AgentEvent::FinalMessage { text }) => {
                let (clean_text, raw_citations) = extract_citations(&text);

                // Emit CitationFound for each unique (doc_id, para_id) pair.
                for (label, (doc_id_str, para_id)) in raw_citations.iter().enumerate() {
                    let label = label + 1;
                    let doc_id = Uuid::parse_str(doc_id_str).unwrap_or(Uuid::nil());

                    let title = if let Some(t) = doc_title_cache.get(&doc_id) {
                        t.clone()
                    } else {
                        state
                            .storage
                            .get_document(doc_id)
                            .await
                            .map(|d| d.title)
                            .unwrap_or_default()
                    };

                    let evt = json!({
                        "type": "CitationFound",
                        "docId": doc_id_str,
                        "paragraphId": para_id,
                        "label": label,
                        "title": title,
                    });
                    send!(evt);
                }

                // Stream the cleaned text in ~80-char chunks.
                for chunk in clean_text
                    .as_bytes()
                    .chunks(80)
                    .map(|c| std::str::from_utf8(c).unwrap_or_default())
                {
                    let evt = json!({"type": "TextChunk", "content": chunk});
                    send!(evt);
                    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
                }
            }

            Ok(_) => {} // ThinkingChunk, ToolCall — not surfaced to UI
        }
    }

    send!(json!({"type": "Done"}));
}

// ── Route: POST /edits/:id/approve ────────────────────────────────────────────

#[derive(Deserialize)]
struct ApproveBody {
    proposed: Option<String>,
}

async fn approve_edit(
    State(state): State<S>,
    AxumPath(edit_id): AxumPath<Uuid>,
    Json(body): Json<ApproveBody>,
) -> Result<Json<Value>, ApiError> {
    let stored = state
        .pending_edits
        .lock()
        .await
        .remove(&edit_id)
        .ok_or_else(|| ApiError::not_found(format!("edit {edit_id}")))?;

    let final_proposed = body.proposed.as_deref().unwrap_or(&stored.proposed_paragraph);

    // If the user modified the proposed paragraph text, rebuild the new_body.
    let edit = if final_proposed != stored.proposed_paragraph {
        apply_user_modification(stored.edit, &stored.proposed_paragraph, final_proposed)
    } else {
        stored.edit
    };

    let session = UserSession { user_id: Uuid::nil() };
    let token = session.approve_edit(&edit);

    commit_edit(edit, token, state.storage.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(json!({"ok": true})))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse `[[doc_id:para_id]]` markers from text, replacing them with `[n]`.
/// Returns the cleaned text and a list of (doc_id, para_id) pairs in order.
fn extract_citations(text: &str) -> (String, Vec<(String, String)>) {
    let mut result = String::with_capacity(text.len());
    let mut citations: Vec<(String, String)> = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("[[") {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..];

        if let Some(end) = remaining.find("]]") {
            let inner = &remaining[..end];
            remaining = &remaining[end + 2..];

            if let Some(colon) = inner.find(':') {
                let doc_id = inner[..colon].trim().to_string();
                let para_id = inner[colon + 1..].trim().to_string();

                // Deduplicate: reuse label if same (doc_id, para_id) seen before.
                let label = citations
                    .iter()
                    .position(|(d, p)| d == &doc_id && p == &para_id)
                    .map(|i| i + 1)
                    .unwrap_or_else(|| {
                        citations.push((doc_id, para_id));
                        citations.len()
                    });

                result.push_str(&format!("[{label}]"));
            } else {
                // Not a citation, restore as-is.
                result.push_str("[[");
                result.push_str(inner);
                result.push_str("]]");
            }
        } else {
            // No closing ]], put back and stop.
            result.push_str("[[");
            result.push_str(remaining);
            remaining = "";
        }
    }

    result.push_str(remaining);
    (result, citations)
}

/// Extract the first changed paragraph from a `ProposedEdit`, store it in
/// `pending_edits`, and return the serialised SSE event string.
async fn build_edit_sse(state: &AppState, edit: &ProposedEdit) -> Option<Value> {
    let (doc_id, para_id, original, proposed) = match &edit.kind {
        ProposedEditKind::Modify { document_id, diff, .. } => {
            let first = diff.changes.iter().find_map(|c| {
                if let ParagraphChange::Modify { paragraph_id, old_text, new_text } = c {
                    Some((paragraph_id.clone(), old_text.clone(), new_text.clone()))
                } else {
                    None
                }
            })?;
            (Some(*document_id), first.0, first.1, first.2)
        }
        ProposedEditKind::Create { title: _, body } => {
            (None, "p0".to_string(), String::new(), body.clone())
        }
        ProposedEditKind::UpdateContent { document_id, new_content } => {
            (Some(*document_id), "p0".to_string(), String::new(), new_content.clone())
        }
        ProposedEditKind::DeleteDocument { .. } => return None,
    };

    let doc_title = if let Some(id) = doc_id {
        state
            .storage
            .get_document(id)
            .await
            .map(|d| d.title)
            .unwrap_or_default()
    } else {
        String::new()
    };

    let stored = StoredEdit {
        edit: edit.clone(),
        doc_title,
        first_paragraph_id: para_id.clone(),
        original_paragraph: original.clone(),
        proposed_paragraph: proposed.clone(),
    };
    state.pending_edits.lock().await.insert(edit.id, stored);

    Some(json!({
        "type": "ProposedEditEmitted",
        "editId": edit.id,
        "docId": doc_id,
        "paragraphId": para_id,
        "original": original,
        "proposed": proposed,
    }))
}

/// Rebuild the edit's new_body with the user's modified paragraph text.
fn apply_user_modification(mut edit: ProposedEdit, original: &str, user_text: &str) -> ProposedEdit {
    if let ProposedEditKind::Modify { ref mut new_body, .. } = edit.kind {
        *new_body = new_body.replacen(original, user_text, 1);
    }
    edit
}

/// Generate a minimal valid single-page PDF for `title`.
fn minimal_pdf(title: &str, body: &str) -> Vec<u8> {
    let safe_title = title.replace(['(', ')', '\\', '\n', '\r'], " ");
    let safe_body: String = body
        .chars()
        .take(300)
        .map(|c| if c == '(' || c == ')' || c == '\\' { ' ' } else { c })
        .collect();

    let stream_text = format!(
        "BT /F1 14 Tf 50 750 Td ({safe_title}) Tj 0 -20 Td /F1 10 Tf ({safe_body}) Tj ET"
    );
    let header = format!(
        "%PDF-1.4\n\
         1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
         2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
         3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R\
                   /Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>endobj\n\
         4 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\n"
    );
    let content = format!(
        "5 0 obj\n<</Length {}>>\nstream\n{stream_text}\nendstream\nendobj\n",
        stream_text.len()
    );
    let body_str = header + &content;
    let xref_offset = body_str.len();
    let trailer = format!(
        "\nxref\n0 6\n0000000000 65535 f \n\
         trailer\n<</Size 6/Root 1 0 R>>\nstartxref\n{xref_offset}\n%%EOF"
    );
    (body_str + &trailer).into_bytes()
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

// ── Error type ────────────────────────────────────────────────────────────────

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: msg.into() }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: msg.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({"error": self.message}))).into_response()
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // ── --schema mode ────────────────────────────────────────────────────────
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

    let storage: Arc<dyn StorageAdapter> = Arc::new(
        SqliteAdapter::new(content_dir, Path::new(&db_path))
            .await
            .expect("open storage"),
    );

    // ── LLM router (optional — chat returns 503 if unavailable) ──────────────
    let config_path = std::env::var("KAYA_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("kaya.yaml"));

    let router: Option<Arc<ModelRouter>> =
        match ModelRouter::from_yaml(&config_path) {
            Ok(r) => {
                eprintln!("kaya-oss: LLM router loaded from {config_path:?}");
                Some(Arc::new(r))
            }
            Err(e) => {
                eprintln!("kaya-oss: LLM router unavailable ({e}); /sessions/:id/chat will return 503");
                None
            }
        };

    // ── Sessions DB (same SQLite file, separate pool) ─────────────────────────
    let sessions_pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await
        .expect("open sessions pool");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id            TEXT    PRIMARY KEY,
            title         TEXT    NOT NULL,
            created_at    INTEGER NOT NULL,
            updated_at    INTEGER NOT NULL,
            message_count INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&sessions_pool)
    .await
    .expect("create sessions table");

    // ── App state ─────────────────────────────────────────────────────────────
    let state: S = Arc::new(AppState {
        storage,
        router,
        sessions_pool,
        pending_edits: Mutex::new(HashMap::new()),
    });

    // ── CORS ──────────────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(AllowHeaders::any());

    // ── Router ────────────────────────────────────────────────────────────────
    let app = oa_router
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/documents", get(list_documents))
        .route("/documents/{id}", get(get_document))
        .route("/documents/{id}/export.pdf", get(export_document_pdf))
        .route("/sessions/{id}/chat", post(chat_stream))
        .route("/edits/{id}/approve", post(approve_edit))
        .with_state(state)
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
