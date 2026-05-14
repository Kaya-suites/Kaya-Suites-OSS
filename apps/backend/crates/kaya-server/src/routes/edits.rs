use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use kaya_core::{ProposedEditKind, StorageAdapter, auth::UserSession, edit::commit_edit, model_router::ModelRouter, retrieval::index_document_chunks};

use crate::error::ApiError;
use crate::state::StoredEdit;

#[derive(Deserialize)]
pub struct ApproveBody {
    pub proposed: Option<String>,
}

pub async fn approve_edit(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Extension(llm): Extension<Option<Arc<ModelRouter>>>,
    Extension(pending_edits): Extension<Arc<Mutex<HashMap<Uuid, StoredEdit>>>>,
    Path(edit_id): Path<Uuid>,
    Json(body): Json<ApproveBody>,
) -> Result<Json<Value>, ApiError> {
    let stored = pending_edits
        .lock()
        .await
        .remove(&edit_id)
        .ok_or_else(|| ApiError::not_found(format!("edit {edit_id}")))?;

    let final_proposed = body.proposed.as_deref().unwrap_or(&stored.proposed_paragraph);

    let edit = if final_proposed != stored.proposed_paragraph {
        apply_user_modification(stored.edit, &stored.proposed_paragraph, final_proposed)
    } else {
        stored.edit
    };

    let session = UserSession { user_id: Uuid::nil() };
    let token = session.approve_edit(&edit);

    let affected_doc_id = commit_edit(edit, token, storage.clone())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if let (Some(doc_id), Some(router)) = (affected_doc_id, llm) {
        let storage = storage.clone();
        tokio::spawn(async move {
            match storage.get_document(doc_id).await {
                Ok(doc) => {
                    if let Err(e) = index_document_chunks(&doc, &storage, &router).await {
                        tracing::error!(document_id = %doc_id, error = %e, "reindex failed after approve");
                    }
                }
                Err(e) => tracing::error!(document_id = %doc_id, error = %e, "get_document failed after approve"),
            }
        });
    }

    Ok(Json(json!({"ok": true})))
}

fn apply_user_modification(
    mut edit: kaya_core::ProposedEdit,
    original: &str,
    user_text: &str,
) -> kaya_core::ProposedEdit {
    if let ProposedEditKind::Modify { ref mut new_body, .. } = edit.kind {
        *new_body = new_body.replacen(original, user_text, 1);
    }
    edit
}
