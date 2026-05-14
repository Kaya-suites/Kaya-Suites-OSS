use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use kaya_core::SessionStorage;

use crate::error::ApiError;

// ── GET /sessions ─────────────────────────────────────────────────────────────

pub async fn list_sessions(
    Extension(sessions): Extension<Arc<dyn SessionStorage>>,
) -> Result<Json<Vec<kaya_core::Session>>, ApiError> {
    let list = sessions
        .list_sessions()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(list))
}

// ── POST /sessions ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateSessionBody {
    pub title: Option<String>,
}

pub async fn create_session(
    Extension(sessions): Extension<Arc<dyn SessionStorage>>,
    Json(body): Json<CreateSessionBody>,
) -> Result<(StatusCode, Json<kaya_core::Session>), ApiError> {
    let session = sessions
        .create_session(body.title)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((StatusCode::CREATED, Json(session)))
}

// ── GET /sessions/:id/messages ────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MessageResponse {
    id: String,
    role: String,
    content: String,
    citations: Value,
    created_at: i64,
}

pub async fn get_session_messages(
    Extension(sessions): Extension<Arc<dyn SessionStorage>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<MessageResponse>>, ApiError> {
    let records = sessions
        .get_messages(session_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let msgs = records
        .into_iter()
        .map(|r| MessageResponse {
            id: r.id,
            role: r.role,
            content: r.content,
            citations: serde_json::from_str(&r.citations_json).unwrap_or(Value::Array(vec![])),
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(msgs))
}
