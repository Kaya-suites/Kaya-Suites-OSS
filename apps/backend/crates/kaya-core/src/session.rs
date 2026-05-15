//! SessionStorage trait and domain types for chat sessions and messages.

use async_trait::async_trait;
use uuid::Uuid;

/// A chat session.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: Uuid,
    pub title: String,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    /// Unix epoch milliseconds.
    pub updated_at: i64,
    pub message_count: u32,
}

/// A persisted chat message.
#[derive(Debug, Clone)]
pub struct MessageRecord {
    pub id: String,
    pub role: String,
    pub content: String,
    /// JSON array string of citation objects.
    pub citations_json: String,
    /// Unix epoch milliseconds.
    pub created_at: i64,
}

/// Error type for session storage operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session not found: {0}")]
    NotFound(Uuid),
    #[error("backend error: {0}")]
    Backend(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// Abstracts over SQLite (OSS) and Postgres (cloud) session backends.
#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn list_sessions(&self) -> Result<Vec<Session>, SessionError>;
    async fn create_session(&self, title: Option<String>) -> Result<Session, SessionError>;
    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<MessageRecord>, SessionError>;
    /// Return (role, content) pairs ordered oldest-first for LLM context.
    async fn get_prior_messages(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<(String, String)>, SessionError>;
    async fn save_user_message(
        &self,
        session_id: Uuid,
        id: &str,
        content: &str,
    ) -> Result<(), SessionError>;
    async fn save_assistant_message(
        &self,
        session_id: Uuid,
        id: &str,
        content: &str,
        citations_json: &str,
    ) -> Result<(), SessionError>;
    /// Update the session's `updated_at` timestamp (and `message_count` where tracked).
    async fn touch_session(&self, session_id: Uuid) -> Result<(), SessionError>;
    /// Rename the session, replacing its current title.
    async fn rename_session(&self, session_id: Uuid, title: String) -> Result<(), SessionError>;
}
