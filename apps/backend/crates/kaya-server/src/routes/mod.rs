use axum::{Router, routing::{get, post}};

mod chat;
mod documents;
mod edits;
mod sessions;

/// Build the shared API router, generic over the host binary's state type.
///
/// All handlers read their dependencies from Axum [`Extension`]s rather than
/// `State`, so the router is compatible with any `S`. The caller must inject
/// the following extensions before requests reach these routes:
///
/// - `Arc<dyn StorageAdapter>` — per-request storage (scoped to user in cloud)
/// - `Arc<dyn SessionStorage>` — per-request session storage
/// - `Option<Arc<ModelRouter>>` — LLM router (None → 503 on chat routes)
/// - `Arc<Mutex<HashMap<Uuid, StoredEdit>>>` — pending edit map
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/documents", get(documents::list_documents).post(documents::create_document))
        .route(
            "/documents/{id}",
            get(documents::get_document)
                .put(documents::update_document)
                .delete(documents::delete_document),
        )
        .route("/documents/{id}/export.pdf", get(documents::export_document_pdf))
        .route("/sessions", get(sessions::list_sessions).post(sessions::create_session))
        .route("/sessions/{id}/messages", get(sessions::get_session_messages))
        .route("/sessions/{id}/chat", post(chat::chat_stream))
        .route("/edits/{id}/approve", post(edits::approve_edit))
}
