//! SQLite-backed `StorageAdapter` implementation for Kaya Suites OSS.
//!
//! # Architecture
//! - Files on disk (`.md` with YAML frontmatter) are the **source of truth**.
//! - SQLite is a **fast index** used for listing and search; it is never the
//!   primary store.
//! - `get_document` always reads from disk; the index is only consulted to
//!   resolve a UUID → file path mapping.
//! - On startup a background task reconciles the index with the current state
//!   of disk so that manual edits are detected (FR-5).

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use sqlx::{
    Row,
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use tokio::sync::watch;
use uuid::Uuid;

use kaya_core::storage::{Document, Embedding, StorageAdapter, StorageError};

use crate::document::{parse_document, sha256_hex, to_markdown};

// ── Inner shared state ────────────────────────────────────────────────────────

struct Inner {
    pool: SqlitePool,
    content_dir: PathBuf,
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// SQLite-backed storage adapter (OSS / Apache 2.0).
///
/// Construct with [`SqliteAdapter::new`]; it is immediately usable while a
/// background reconciliation task runs.
pub struct SqliteAdapter {
    inner: Arc<Inner>,
    /// Receiver that becomes `true` once the first reconciliation pass finishes.
    reconciled_rx: watch::Receiver<bool>,
}

impl SqliteAdapter {
    /// Open (or create) the SQLite database at `db_path` and use `content_dir`
    /// as the on-disk document store.
    ///
    /// Reconciliation starts in the background immediately; call
    /// [`wait_for_reconciliation`](Self::wait_for_reconciliation) in tests when
    /// you need to be sure the index reflects the current state of disk.
    pub async fn new(content_dir: PathBuf, db_path: &Path) -> Result<Self, StorageError> {
        tokio::fs::create_dir_all(&content_dir)
            .await
            .map_err(box_err)?;

        let opts = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .connect_with(opts)
            .await
            .map_err(box_err)?;

        run_migrations(&pool).await?;

        let inner = Arc::new(Inner {
            pool,
            content_dir,
        });

        let (tx, rx) = watch::channel(false);
        let inner_bg = Arc::clone(&inner);
        tokio::spawn(async move {
            if let Err(e) = reconcile(&inner_bg).await {
                eprintln!("[kaya-storage] reconciliation error: {e:#}");
            }
            // Ignore send error — receiver may have been dropped in tests.
            let _ = tx.send(true);
        });

        Ok(Self {
            inner,
            reconciled_rx: rx,
        })
    }

    /// Block until the initial reconciliation pass has finished.
    ///
    /// Used in integration tests to assert post-reconciliation state.
    pub async fn wait_for_reconciliation(&self) {
        let mut rx = self.reconciled_rx.clone();
        // `wait_for` checks the current value first, so this returns immediately
        // if reconciliation already completed before we called this.
        rx.wait_for(|&done| done).await.unwrap();
    }
}

// ── StorageAdapter impl ───────────────────────────────────────────────────────

#[async_trait]
impl StorageAdapter for SqliteAdapter {
    /// Read a document from disk. The index is consulted only to resolve the
    /// UUID → relative file path; the file itself is the authoritative source.
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query(
            "SELECT path, deleted_at FROM documents WHERE id = ?",
        )
        .bind(&id_str)
        .fetch_optional(&self.inner.pool)
        .await
        .map_err(box_err)?;

        let row = row.ok_or(StorageError::NotFound(id))?;

        let deleted_at: Option<String> = row.try_get("deleted_at").map_err(box_err)?;
        if deleted_at.is_some() {
            return Err(StorageError::NotFound(id));
        }

        let rel_path: String = row.try_get("path").map_err(box_err)?;
        let abs_path = self.inner.content_dir.join(&rel_path);

        let raw = tokio::fs::read_to_string(&abs_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(id)
            } else {
                box_err(e)
            }
        })?;

        let (mut doc, _) = parse_document(&raw).map_err(box_err)?;
        doc.path = Some(PathBuf::from(rel_path));
        Ok(doc)
    }

    /// Write a document to disk and update the index.
    ///
    /// If `doc.path` is `None` the adapter assigns `<uuid>.md` as the relative
    /// path. The UUID is stable — renaming the file later does not change it.
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError> {
        let rel_path = doc
            .path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("{}.md", doc.id)));

        let abs_path = self.inner.content_dir.join(&rel_path);
        if let Some(parent) = abs_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(box_err)?;
        }

        let raw = to_markdown(doc).map_err(box_err)?;
        tokio::fs::write(&abs_path, raw.as_bytes())
            .await
            .map_err(box_err)?;

        let hash = sha256_hex(raw.as_bytes());
        let rel_str = rel_path.to_string_lossy().to_string();
        upsert_index(&self.inner.pool, doc, &rel_str, &hash).await?;
        Ok(())
    }

    /// Remove a document from disk and mark it deleted in the index.
    ///
    /// No-op if the document does not exist.
    async fn delete_document(&self, id: Uuid) -> Result<(), StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query(
            "SELECT path FROM documents WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&id_str)
        .fetch_optional(&self.inner.pool)
        .await
        .map_err(box_err)?;

        if let Some(row) = row {
            let path: String = row.try_get("path").map_err(box_err)?;
            let abs_path = self.inner.content_dir.join(&path);
            // Best-effort: ignore NotFound; the index will be marked deleted.
            let _ = tokio::fs::remove_file(&abs_path).await;

            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "UPDATE documents SET deleted_at = ? WHERE id = ?",
            )
            .bind(&now)
            .bind(&id_str)
            .execute(&self.inner.pool)
            .await
            .map_err(box_err)?;
        }

        Ok(())
    }

    /// Return all non-deleted documents by reading each file from disk.
    ///
    /// Files that have disappeared from disk since the last reconciliation are
    /// silently skipped; the next reconciliation will mark them deleted.
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError> {
        let rows =
            sqlx::query("SELECT id, path FROM documents WHERE deleted_at IS NULL ORDER BY updated_at DESC")
                .fetch_all(&self.inner.pool)
                .await
                .map_err(box_err)?;

        let mut docs = Vec::with_capacity(rows.len());
        for row in rows {
            let rel_path: String = row.try_get("path").map_err(box_err)?;
            let abs_path = self.inner.content_dir.join(&rel_path);

            match tokio::fs::read_to_string(&abs_path).await {
                Ok(raw) => {
                    let (mut doc, _) = parse_document(&raw).map_err(box_err)?;
                    doc.path = Some(PathBuf::from(rel_path));
                    docs.push(doc);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // File gone between reconciliation passes; skip.
                }
                Err(e) => return Err(box_err(e)),
            }
        }
        Ok(docs)
    }

    /// Not yet implemented (next prompt). Always returns an empty list.
    async fn search_embeddings(
        &self,
        _query: &[f32],
        _limit: usize,
    ) -> Result<Vec<Embedding>, StorageError> {
        Ok(vec![])
    }

    /// Not yet implemented (next prompt). No-op.
    async fn save_embeddings(&self, _embedding: &Embedding) -> Result<(), StorageError> {
        Ok(())
    }
}

// ── Migrations ────────────────────────────────────────────────────────────────

async fn run_migrations(pool: &SqlitePool) -> Result<(), StorageError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS documents (
            id               TEXT PRIMARY KEY,
            title            TEXT NOT NULL,
            path             TEXT NOT NULL UNIQUE,
            frontmatter_json TEXT NOT NULL,
            content_hash     TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            deleted_at       TEXT
        )",
    )
    .execute(pool)
    .await
    .map_err(box_err)?;
    Ok(())
}

// ── Reconciliation ─────────────────────────────────────────────────────────────

/// Scan the content directory and synchronise the index.
///
/// - New or changed files are (re)indexed.
/// - Files that disappeared from disk are marked deleted in the index.
///
/// Runs inside a background `tokio::spawn` task; errors are logged but do not
/// propagate to the caller.
async fn reconcile(inner: &Arc<Inner>) -> anyhow::Result<()> {
    let content_dir = inner.content_dir.clone();

    // Collect .md paths synchronously on a blocking thread.
    let rel_paths: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
        let mut acc = Vec::new();
        for entry in walkdir::WalkDir::new(&content_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().extension().map_or(false, |ext| ext == "md") {
                if let Ok(rel) = entry.path().strip_prefix(&content_dir) {
                    acc.push(rel.to_path_buf());
                }
            }
        }
        acc
    })
    .await?;

    let mut disk_paths: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(rel_paths.len());

    for rel_path in rel_paths {
        let rel_str = rel_path.to_string_lossy().to_string();
        disk_paths.insert(rel_str.clone());
        let abs_path = inner.content_dir.join(&rel_path);

        let raw = match tokio::fs::read_to_string(&abs_path).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let hash = sha256_hex(raw.as_bytes());

        let row = sqlx::query(
            "SELECT id, content_hash FROM documents WHERE path = ? AND deleted_at IS NULL",
        )
        .bind(&rel_str)
        .fetch_optional(&inner.pool)
        .await?;

        let needs_index = match &row {
            Some(r) => {
                let indexed_hash: String = r.try_get("content_hash")?;
                indexed_hash != hash
            }
            None => true,
        };

        if needs_index {
            process_file(inner, &abs_path, &rel_str, raw, hash).await?;
        }
    }

    // Mark deleted any non-deleted index entries whose files are gone.
    let rows =
        sqlx::query("SELECT id, path FROM documents WHERE deleted_at IS NULL")
            .fetch_all(&inner.pool)
            .await?;

    let now = chrono::Utc::now().to_rfc3339();
    for row in rows {
        let path: String = row.try_get("path")?;
        if !disk_paths.contains(&path) {
            let id: String = row.try_get("id")?;
            sqlx::query("UPDATE documents SET deleted_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&id)
                .execute(&inner.pool)
                .await?;
        }
    }

    Ok(())
}

/// Parse a file, optionally write back an assigned UUID, then upsert the index.
async fn process_file(
    inner: &Arc<Inner>,
    abs_path: &Path,
    rel_str: &str,
    raw: String,
    mut hash: String,
) -> anyhow::Result<()> {
    let (mut doc, id_generated) = match parse_document(&raw) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[kaya-storage] skipping {rel_str}: {e}");
            return Ok(());
        }
    };
    doc.path = Some(PathBuf::from(rel_str));

    // FR-2: if the file had no id, write it back so the UUID is stable.
    if id_generated {
        match to_markdown(&doc) {
            Ok(updated) => {
                if tokio::fs::write(abs_path, updated.as_bytes()).await.is_ok() {
                    hash = sha256_hex(updated.as_bytes());
                }
            }
            Err(e) => eprintln!("[kaya-storage] could not serialise {rel_str}: {e}"),
        }
    }

    upsert_index(&inner.pool, &doc, rel_str, &hash).await?;
    Ok(())
}

// ── Index helpers ─────────────────────────────────────────────────────────────

/// Insert or update the index row for a document.
///
/// If another row claims the same `path` but a different `id` (e.g. the file
/// was replaced), that stale row is removed first so the UNIQUE constraint
/// on `path` is not violated.
async fn upsert_index(
    pool: &SqlitePool,
    doc: &Document,
    rel_path: &str,
    hash: &str,
) -> Result<(), StorageError> {
    let id_str = doc.id.to_string();

    // Remove any stale entry that owns this path with a different id.
    sqlx::query("DELETE FROM documents WHERE path = ? AND id != ?")
        .bind(rel_path)
        .bind(&id_str)
        .execute(pool)
        .await
        .map_err(box_err)?;

    let fm_json = serde_json::to_string(&doc).map_err(box_err)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO documents (id, title, path, frontmatter_json, content_hash, updated_at, deleted_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL)
         ON CONFLICT(id) DO UPDATE SET
           title            = excluded.title,
           path             = excluded.path,
           frontmatter_json = excluded.frontmatter_json,
           content_hash     = excluded.content_hash,
           updated_at       = excluded.updated_at,
           deleted_at       = NULL",
    )
    .bind(&id_str)
    .bind(&doc.title)
    .bind(rel_path)
    .bind(&fm_json)
    .bind(hash)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(box_err)?;

    Ok(())
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> StorageError {
    StorageError::Backend(Box::new(e))
}
