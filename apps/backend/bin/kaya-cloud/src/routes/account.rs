// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Account management routes (FR-30, FR-31).
//!
//! - `POST /account/delete` — hard-delete all user data (FR-30)
//! - `GET  /account/export` — ZIP archive of all user data (FR-31)

use std::io::{Cursor, Write as _};

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use kaya_tenant::{AuthSession, KayaAuthBackend};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/account/delete", post(delete_account))
        .route("/account/export", get(export_account))
}

// ── POST /account/delete ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DeleteAccountBody {
    /// Must equal `"DELETE MY ACCOUNT"` (FR-30 confirmation gate).
    confirm: String,
}

#[derive(Serialize)]
struct DeleteResponse {
    ok: bool,
}

async fn delete_account(
    State(pool): State<PgPool>,
    mut auth: AuthSession<KayaAuthBackend>,
    Json(body): Json<DeleteAccountBody>,
) -> Response {
    let user = match auth.user.clone() {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    if body.confirm != "DELETE MY ACCOUNT" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "confirmation phrase mismatch"})),
        )
            .into_response();
    }

    // Deleting the users row cascades to all user-scoped tables (FK ON DELETE CASCADE).
    match sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id)
        .execute(&pool)
        .await
    {
        Ok(_) => {}
        Err(e) => {
            tracing::error!(user_id = %user.id, error = %e, "account deletion failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }

    // Destroy the session so the cookie is immediately invalid.
    let _ = auth.logout().await;

    tracing::info!(user_id = %user.id, "account deleted");
    (StatusCode::OK, Json(DeleteResponse { ok: true })).into_response()
}

// ── GET /account/export ───────────────────────────────────────────────────────

async fn export_account(
    State(pool): State<PgPool>,
    auth: AuthSession<KayaAuthBackend>,
) -> Response {
    let user = match auth.user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match build_export(&pool, user.id, &user.email).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/zip"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"kaya-export.zip\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(user_id = %user.id, error = %e, "export failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── Export helpers ────────────────────────────────────────────────────────────

#[derive(Debug)]
struct DocRow {
    #[allow(dead_code)]
    id: Uuid,
    title: String,
    body: String,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct ChatMsg {
    role: String,
    content: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct ChatSession {
    id: Uuid,
    title: Option<String>,
    messages: Vec<ChatMsg>,
}

async fn build_export(
    pool: &PgPool,
    user_id: Uuid,
    email: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch documents.
    let doc_rows = sqlx::query(
        "SELECT id, title, body, updated_at
         FROM documents
         WHERE user_id = $1 AND deleted_at IS NULL
         ORDER BY updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let docs: Vec<DocRow> = doc_rows
        .iter()
        .map(|r| DocRow {
            id: r.try_get("id").unwrap(),
            title: r.try_get("title").unwrap(),
            body: r.try_get("body").unwrap(),
            updated_at: r.try_get("updated_at").unwrap(),
        })
        .collect();

    // Fetch chat sessions + messages.
    let session_rows = sqlx::query(
        "SELECT id, title FROM chat_sessions WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut chat_sessions: Vec<ChatSession> = Vec::new();
    for sr in &session_rows {
        let sid: Uuid = sr.try_get("id").unwrap();
        let msg_rows = sqlx::query(
            "SELECT role, content, created_at
             FROM chat_messages
             WHERE session_id = $1 AND user_id = $2
             ORDER BY created_at ASC",
        )
        .bind(sid)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let messages = msg_rows
            .iter()
            .map(|m| ChatMsg {
                role: m.try_get("role").unwrap(),
                content: m.try_get("content").unwrap(),
                created_at: m
                    .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                    .unwrap()
                    .to_rfc3339(),
            })
            .collect();

        chat_sessions.push(ChatSession {
            id: sid,
            title: sr.try_get("title").unwrap_or(None),
            messages,
        });
    }

    let email_owned = email.to_owned();
    let now = chrono::Utc::now();

    // ZIP creation is synchronous — run in a blocking thread.
    let bytes = tokio::task::spawn_blocking(move || {
        create_zip(&docs, &chat_sessions, &email_owned, now)
    })
    .await??;

    Ok(bytes)
}

fn create_zip(
    docs: &[DocRow],
    chats: &[ChatSession],
    email: &str,
    exported_at: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let opts =
        zip::write::FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

    // manifest.json
    zip.start_file("manifest.json", opts)?;
    let manifest = serde_json::json!({
        "exported_at": exported_at.to_rfc3339(),
        "email": email,
        "document_count": docs.len(),
        "chat_session_count": chats.len(),
    });
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    // documents/{title}.md
    for doc in docs {
        let safe_title = sanitize_filename(&doc.title);
        let filename = format!("documents/{safe_title}.md");
        zip.start_file(&filename, opts)?;
        let header = format!(
            "# {}\n\n> Last updated: {}\n\n",
            doc.title,
            doc.updated_at.format("%Y-%m-%d")
        );
        zip.write_all(header.as_bytes())?;
        zip.write_all(doc.body.as_bytes())?;
    }

    // chat_history.json
    zip.start_file("chat_history.json", opts)?;
    zip.write_all(serde_json::to_string_pretty(chats)?.as_bytes())?;

    let inner = zip.finish()?;
    Ok(inner.into_inner())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .take(64)
        .collect()
}
