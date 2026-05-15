//! SQLite-backed [`SessionStorage`] implementation.

use async_trait::async_trait;
use chrono::Utc;
use kaya_core::session::{MessageRecord, Session, SessionError, SessionStorage};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// SQLite session storage for the OSS single-user binary.
pub struct SqliteSessionStorage {
    pool: SqlitePool,
}

impl SqliteSessionStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create the `sessions` and `messages` tables if they do not exist.
    pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id            TEXT    PRIMARY KEY,
                title         TEXT    NOT NULL,
                created_at    INTEGER NOT NULL,
                updated_at    INTEGER NOT NULL,
                message_count INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id         TEXT    PRIMARY KEY,
                session_id TEXT    NOT NULL,
                role       TEXT    NOT NULL,
                content    TEXT    NOT NULL,
                citations  TEXT    NOT NULL DEFAULT '[]',
                created_at INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> SessionError {
    SessionError::Backend(Box::new(e))
}

#[async_trait]
impl SessionStorage for SqliteSessionStorage {
    async fn list_sessions(&self) -> Result<Vec<Session>, SessionError> {
        let rows = sqlx::query(
            "SELECT id, title, created_at, updated_at, message_count
             FROM sessions ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(box_err)?;

        rows.into_iter()
            .map(|row| -> Result<Session, SessionError> {
                Ok(Session {
                    id: Uuid::parse_str(row.try_get::<&str, _>("id").map_err(box_err)?)
                        .map_err(|e| SessionError::Backend(Box::new(e)))?,
                    title: row.try_get("title").map_err(box_err)?,
                    created_at: row.try_get("created_at").map_err(box_err)?,
                    updated_at: row.try_get("updated_at").map_err(box_err)?,
                    message_count: row.try_get::<i64, _>("message_count").map_err(box_err)? as u32,
                })
            })
            .collect()
    }

    async fn create_session(&self, title: Option<String>) -> Result<Session, SessionError> {
        let id = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();
        let title = title.unwrap_or_else(|| "New conversation".to_string());

        sqlx::query(
            "INSERT INTO sessions (id, title, created_at, updated_at, message_count)
             VALUES (?, ?, ?, ?, 0)",
        )
        .bind(id.to_string())
        .bind(&title)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(box_err)?;

        Ok(Session { id, title, created_at: now, updated_at: now, message_count: 0 })
    }

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<MessageRecord>, SessionError> {
        let rows = sqlx::query(
            "SELECT id, role, content, citations, created_at
             FROM messages WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(session_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(box_err)?;

        rows.into_iter()
            .map(|row| -> Result<MessageRecord, SessionError> {
                Ok(MessageRecord {
                    id: row.try_get("id").map_err(box_err)?,
                    role: row.try_get("role").map_err(box_err)?,
                    content: row.try_get("content").map_err(box_err)?,
                    citations_json: row.try_get("citations").map_err(box_err)?,
                    created_at: row.try_get("created_at").map_err(box_err)?,
                })
            })
            .collect()
    }

    async fn get_prior_messages(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<(String, String)>, SessionError> {
        let rows = sqlx::query(
            "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(session_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(box_err)?;

        rows.into_iter()
            .map(|row| -> Result<(String, String), SessionError> {
                Ok((
                    row.try_get("role").map_err(box_err)?,
                    row.try_get("content").map_err(box_err)?,
                ))
            })
            .collect()
    }

    async fn save_user_message(
        &self,
        session_id: Uuid,
        id: &str,
        content: &str,
    ) -> Result<(), SessionError> {
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, citations, created_at)
             VALUES (?, ?, 'user', ?, '[]', ?)",
        )
        .bind(id)
        .bind(session_id.to_string())
        .bind(content)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(box_err)?;
        Ok(())
    }

    async fn save_assistant_message(
        &self,
        session_id: Uuid,
        id: &str,
        content: &str,
        citations_json: &str,
    ) -> Result<(), SessionError> {
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, citations, created_at)
             VALUES (?, ?, 'assistant', ?, ?, ?)",
        )
        .bind(id)
        .bind(session_id.to_string())
        .bind(content)
        .bind(citations_json)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(box_err)?;
        Ok(())
    }

    async fn touch_session(&self, session_id: Uuid) -> Result<(), SessionError> {
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            "UPDATE sessions SET message_count = message_count + 1, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(session_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(box_err)?;
        Ok(())
    }

    async fn rename_session(&self, session_id: Uuid, title: String) -> Result<(), SessionError> {
        sqlx::query("UPDATE sessions SET title = ? WHERE id = ?")
            .bind(&title)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(box_err)?;
        Ok(())
    }
}
