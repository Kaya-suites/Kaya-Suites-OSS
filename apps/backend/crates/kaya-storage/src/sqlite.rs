//! SQLite-backed `StorageAdapter` implementation for Kaya Suites OSS.
//!
//! # Architecture
//! - SQLite is the **source of truth** for document bodies (stored in the
//!   `body` column of the `documents` table).
//! - On first startup, existing `.md` files in `content_dir` are imported
//!   into the DB by the reconciliation pass; after that, disk files are not
//!   written or read for normal operations.
//! - `get_document` and `list_documents` read exclusively from the DB.
//!   A disk fall-back is retained only for rows that pre-date the `body`
//!   column (i.e. `body IS NULL`) so that existing installations migrate
//!   gracefully on the next reconciliation pass.
//! - On startup a background task reconciles the index with the current state
//!   of disk so that manually added `.md` files are detected (FR-5).
//!
//! # Vector search implementation note
//! Embeddings are stored as packed-f32 BLOBs (little-endian).  At query time
//! all vectors are loaded and cosine similarity is computed in Rust.  This is
//! sufficient for a 1,000-document corpus (~8,000 chunks, ~47 MB).
//!
//! **Production swap**: replace the `chunk_embeddings` regular table with a
//! `sqlite-vec` `vec0` virtual table and change `search_embeddings` to:
//! ```sql
//! SELECT paragraph_id, distance
//! FROM vec_chunks
//! WHERE embedding MATCH ?
//! ORDER BY distance
//! LIMIT ?
//! ```

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

use kaya_core::storage::{Chunk, ChunkHit, Document, Embedding, StorageAdapter, StorageError};

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
    /// Becomes `true` once the first reconciliation pass finishes.
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

        let inner = Arc::new(Inner { pool, content_dir });

        let (tx, rx) = watch::channel(false);
        let inner_bg = Arc::clone(&inner);
        tokio::spawn(async move {
            if let Err(e) = reconcile(&inner_bg).await {
                eprintln!("[kaya-storage] reconciliation error: {e:#}");
            }
            let _ = tx.send(true);
        });

        Ok(Self { inner, reconciled_rx: rx })
    }

    /// Block until the initial reconciliation pass has finished.
    pub async fn wait_for_reconciliation(&self) {
        let mut rx = self.reconciled_rx.clone();
        rx.wait_for(|&done| done).await.unwrap();
    }
}

// ── StorageAdapter impl ───────────────────────────────────────────────────────

#[async_trait]
impl StorageAdapter for SqliteAdapter {
    // ── Documents ─────────────────────────────────────────────────────────────

    /// Read a document from the database. Falls back to disk for rows that
    /// pre-date the `body` column (i.e. `body IS NULL`).
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query(
            "SELECT path, frontmatter_json, body, deleted_at FROM documents WHERE id = ?",
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
        let db_body: Option<String> = row.try_get("body").map_err(box_err)?;

        if let Some(body) = db_body {
            // Body is in the DB — reconstruct from stored JSON + body column.
            let fm_json: String = row.try_get("frontmatter_json").map_err(box_err)?;
            let mut doc: Document = serde_json::from_str(&fm_json).map_err(box_err)?;
            doc.body = body;
            doc.path = Some(PathBuf::from(rel_path));
            return Ok(doc);
        }

        // Legacy fallback: read from disk for rows that don't have body in DB yet.
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

    /// Persist a document to the database. No disk write is performed.
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError> {
        let rel_path = doc
            .path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("{}.md", doc.id)));

        let rel_str = rel_path.to_string_lossy().to_string();
        let hash = sha256_hex(doc.body.as_bytes());
        upsert_index(&self.inner.pool, doc, &rel_str, &hash).await?;
        Ok(())
    }

    /// Remove a document from disk, mark it deleted in the index, and delete
    /// all its chunks and embeddings.
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
            let _ = tokio::fs::remove_file(&abs_path).await;

            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query("UPDATE documents SET deleted_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&id_str)
                .execute(&self.inner.pool)
                .await
                .map_err(box_err)?;

            // Remove all chunks and vector embeddings for this document.
            self.delete_chunks_for_document(id).await?;
            sqlx::query("DELETE FROM chunk_embeddings WHERE document_id = ?")
                .bind(&id_str)
                .execute(&self.inner.pool)
                .await
                .map_err(box_err)?;
        }

        Ok(())
    }

    /// Return all non-deleted documents from the database.
    /// Falls back to disk for any row whose `body` column is still NULL
    /// (pre-migration entries that haven't been reconciled yet).
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError> {
        let rows = sqlx::query(
            "SELECT path, frontmatter_json, body \
             FROM documents WHERE deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .fetch_all(&self.inner.pool)
        .await
        .map_err(box_err)?;

        let mut docs = Vec::with_capacity(rows.len());
        for row in rows {
            let rel_path: String = row.try_get("path").map_err(box_err)?;
            let db_body: Option<String> = row.try_get("body").map_err(box_err)?;

            if let Some(body) = db_body {
                let fm_json: String = row.try_get("frontmatter_json").map_err(box_err)?;
                let mut doc: Document = serde_json::from_str(&fm_json).map_err(box_err)?;
                doc.body = body;
                doc.path = Some(PathBuf::from(rel_path));
                docs.push(doc);
            } else {
                // Legacy fallback for rows without body in DB.
                let abs_path = self.inner.content_dir.join(&rel_path);
                match tokio::fs::read_to_string(&abs_path).await {
                    Ok(raw) => {
                        let (mut doc, _) = parse_document(&raw).map_err(box_err)?;
                        doc.path = Some(PathBuf::from(rel_path));
                        docs.push(doc);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(box_err(e)),
                }
            }
        }
        Ok(docs)
    }

    // ── Chunks ────────────────────────────────────────────────────────────────

    /// Store a chunk in the metadata table and the FTS5 full-text index.
    async fn save_chunk(&self, chunk: &Chunk) -> Result<(), StorageError> {
        let doc_id = chunk.document_id.to_string();
        let content_hash = sha256_hex(chunk.content.as_bytes());

        sqlx::query(
            "INSERT OR REPLACE INTO chunks
             (document_id, paragraph_id, ordinal, content, content_hash)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&doc_id)
        .bind(&chunk.paragraph_id)
        .bind(chunk.ordinal as i64)
        .bind(&chunk.content)
        .bind(&content_hash)
        .execute(&self.inner.pool)
        .await
        .map_err(box_err)?;

        // FTS5 does not support UPSERT; we rely on delete_chunks_for_document
        // being called before save_chunk when re-indexing.
        sqlx::query(
            "INSERT INTO chunk_fts (content, document_id, paragraph_id, ordinal)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&chunk.content)
        .bind(&doc_id)
        .bind(&chunk.paragraph_id)
        .bind(chunk.ordinal as i64)
        .execute(&self.inner.pool)
        .await
        .map_err(box_err)?;

        Ok(())
    }

    /// Delete all chunks (metadata + FTS5 rows) for a document.
    async fn delete_chunks_for_document(&self, document_id: Uuid) -> Result<(), StorageError> {
        let doc_id = document_id.to_string();

        sqlx::query("DELETE FROM chunks WHERE document_id = ?")
            .bind(&doc_id)
            .execute(&self.inner.pool)
            .await
            .map_err(box_err)?;

        // FTS5 supports DELETE with a WHERE on UNINDEXED columns via full scan.
        sqlx::query("DELETE FROM chunk_fts WHERE document_id = ?")
            .bind(&doc_id)
            .execute(&self.inner.pool)
            .await
            .map_err(box_err)?;

        Ok(())
    }

    /// Return `(paragraph_id, content_hash)` pairs for all chunks of a document.
    async fn get_chunk_hashes(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<(String, String)>, StorageError> {
        let doc_id = document_id.to_string();
        let rows = sqlx::query(
            "SELECT paragraph_id, content_hash FROM chunks WHERE document_id = ?",
        )
        .bind(&doc_id)
        .fetch_all(&self.inner.pool)
        .await
        .map_err(box_err)?;

        rows.into_iter()
            .map(|row| {
                let para_id: String = row.try_get("paragraph_id").map_err(box_err)?;
                let hash: String = row.try_get("content_hash").map_err(box_err)?;
                Ok((para_id, hash))
            })
            .collect()
    }

    /// BM25 full-text search via SQLite FTS5. Returns chunks ranked by relevance.
    async fn search_text(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ChunkHit>, StorageError> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        // FTS5 rank is a negative BM25 score; ORDER BY rank ASC = most relevant first.
        let rows = sqlx::query(
            "SELECT document_id, paragraph_id, content, ordinal
             FROM chunk_fts
             WHERE chunk_fts MATCH ?
             ORDER BY rank
             LIMIT ?",
        )
        .bind(query)
        .bind(limit as i64)
        .fetch_all(&self.inner.pool)
        .await
        .map_err(box_err)?;

        rows.into_iter()
            .map(|row| {
                let doc_id_str: String = row.try_get("document_id").map_err(box_err)?;
                let doc_id = Uuid::parse_str(&doc_id_str).map_err(box_err)?;
                let para_id: String = row.try_get("paragraph_id").map_err(box_err)?;
                let content: String = row.try_get("content").map_err(box_err)?;
                let ordinal: i64 = row.try_get("ordinal").map_err(box_err)?;
                Ok(ChunkHit {
                    document_id: doc_id,
                    paragraph_id: para_id,
                    content,
                    ordinal: ordinal as u32,
                })
            })
            .collect()
    }

    // ── Embeddings ────────────────────────────────────────────────────────────

    /// Persist a vector embedding as a packed-f32 BLOB.
    async fn save_embeddings(&self, embedding: &Embedding) -> Result<(), StorageError> {
        let doc_id = embedding.document_id.to_string();
        let blob = encode_f32(&embedding.vector);

        sqlx::query(
            "INSERT OR REPLACE INTO chunk_embeddings (document_id, paragraph_id, vector)
             VALUES (?, ?, ?)",
        )
        .bind(&doc_id)
        .bind(&embedding.paragraph_id)
        .bind(&blob)
        .execute(&self.inner.pool)
        .await
        .map_err(box_err)?;

        Ok(())
    }

    /// Delete embeddings for specific (document_id, paragraph_id) pairs.
    async fn delete_embeddings_for_paragraphs(
        &self,
        document_id: Uuid,
        paragraph_ids: &[String],
    ) -> Result<(), StorageError> {
        if paragraph_ids.is_empty() {
            return Ok(());
        }
        let doc_id = document_id.to_string();
        for para_id in paragraph_ids {
            sqlx::query(
                "DELETE FROM chunk_embeddings WHERE document_id = ? AND paragraph_id = ?",
            )
            .bind(&doc_id)
            .bind(para_id)
            .execute(&self.inner.pool)
            .await
            .map_err(box_err)?;
        }
        Ok(())
    }

    /// Cosine-similarity vector search over all stored embeddings.
    ///
    /// All vectors are loaded and similarity is computed in Rust.  For ≤10,000
    /// chunks this is sub-millisecond; swap to sqlite-vec for larger corpora.
    async fn search_embeddings(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<ChunkHit>, StorageError> {
        if query.is_empty() {
            return Ok(vec![]);
        }

        // Load all embeddings joined with chunk content.
        let rows = sqlx::query(
            "SELECT ce.document_id, ce.paragraph_id, ce.vector,
                    c.content, c.ordinal
             FROM chunk_embeddings ce
             JOIN chunks c
               ON c.document_id = ce.document_id
              AND c.paragraph_id = ce.paragraph_id",
        )
        .fetch_all(&self.inner.pool)
        .await
        .map_err(box_err)?;

        // Compute cosine similarities and sort.
        let mut scored: Vec<(f32, ChunkHit)> = rows
            .into_iter()
            .filter_map(|row| {
                let doc_id_str: String = row.try_get("document_id").ok()?;
                let doc_id = Uuid::parse_str(&doc_id_str).ok()?;
                let para_id: String = row.try_get("paragraph_id").ok()?;
                let blob: Vec<u8> = row.try_get("vector").ok()?;
                let content: String = row.try_get("content").ok()?;
                let ordinal: i64 = row.try_get("ordinal").ok()?;

                let vec = decode_f32(&blob);
                let sim = cosine_similarity(query, &vec);

                Some((
                    sim,
                    ChunkHit {
                        document_id: doc_id,
                        paragraph_id: para_id,
                        content,
                        ordinal: ordinal as u32,
                    },
                ))
            })
            .collect();

        scored.sort_unstable_by(|a, b| {
            b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(scored.into_iter().take(limit).map(|(_, hit)| hit).collect())
    }
}

// ── Migrations ────────────────────────────────────────────────────────────────

async fn run_migrations(pool: &SqlitePool) -> Result<(), StorageError> {
    // Document index (FR-2, FR-5)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS documents (
            id               TEXT PRIMARY KEY,
            title            TEXT NOT NULL,
            path             TEXT NOT NULL UNIQUE,
            frontmatter_json TEXT NOT NULL,
            content_hash     TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            deleted_at       TEXT,
            body             TEXT
        )",
    )
    .execute(pool)
    .await
    .map_err(box_err)?;

    // Add body column to existing databases that pre-date this migration.
    // SQLite returns an error if the column already exists; we ignore it.
    let _ = sqlx::query("ALTER TABLE documents ADD COLUMN body TEXT")
        .execute(pool)
        .await;

    // Chunk metadata + content hashes (used by re-embedding efficiency check, FR-6)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS chunks (
            document_id  TEXT NOT NULL,
            paragraph_id TEXT NOT NULL,
            ordinal      INTEGER NOT NULL,
            content      TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            PRIMARY KEY (document_id, paragraph_id)
        )",
    )
    .execute(pool)
    .await
    .map_err(box_err)?;

    // FTS5 full-text index for BM25 retrieval (FR-7)
    // tokenize='unicode61' is the standard Unicode tokenizer without stemming.
    sqlx::query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS chunk_fts USING fts5(
            content,
            document_id  UNINDEXED,
            paragraph_id UNINDEXED,
            ordinal      UNINDEXED,
            tokenize     = 'unicode61'
        )",
    )
    .execute(pool)
    .await
    .map_err(box_err)?;

    // Vector embeddings stored as packed-f32 BLOBs (little-endian).
    // TODO: replace with sqlite-vec vec0 virtual table for ANN at scale.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS chunk_embeddings (
            document_id  TEXT NOT NULL,
            paragraph_id TEXT NOT NULL,
            vector       BLOB NOT NULL,
            PRIMARY KEY (document_id, paragraph_id)
        )",
    )
    .execute(pool)
    .await
    .map_err(box_err)?;

    Ok(())
}

// ── Reconciliation ─────────────────────────────────────────────────────────────

async fn reconcile(inner: &Arc<Inner>) -> anyhow::Result<()> {
    let content_dir = inner.content_dir.clone();

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

    let rows = sqlx::query("SELECT id, path FROM documents WHERE deleted_at IS NULL")
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

async fn upsert_index(
    pool: &SqlitePool,
    doc: &Document,
    rel_path: &str,
    hash: &str,
) -> Result<(), StorageError> {
    let id_str = doc.id.to_string();

    sqlx::query("DELETE FROM documents WHERE path = ? AND id != ?")
        .bind(rel_path)
        .bind(&id_str)
        .execute(pool)
        .await
        .map_err(box_err)?;

    let fm_json = serde_json::to_string(&doc).map_err(box_err)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO documents (id, title, path, frontmatter_json, content_hash, updated_at, deleted_at, body)
         VALUES (?, ?, ?, ?, ?, ?, NULL, ?)
         ON CONFLICT(id) DO UPDATE SET
           title            = excluded.title,
           path             = excluded.path,
           frontmatter_json = excluded.frontmatter_json,
           content_hash     = excluded.content_hash,
           updated_at       = excluded.updated_at,
           deleted_at       = NULL,
           body             = excluded.body",
    )
    .bind(&id_str)
    .bind(&doc.title)
    .bind(rel_path)
    .bind(&fm_json)
    .bind(hash)
    .bind(&now)
    .bind(&doc.body)
    .execute(pool)
    .await
    .map_err(box_err)?;

    Ok(())
}

// ── Vector helpers ────────────────────────────────────────────────────────────

/// Pack a `Vec<f32>` as a little-endian byte array.
fn encode_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Unpack a little-endian byte array into `Vec<f32>`.
fn decode_f32(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Cosine similarity between two equal-length vectors.  Returns 0 for zero vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn box_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> StorageError {
    StorageError::Backend(Box::new(e))
}
