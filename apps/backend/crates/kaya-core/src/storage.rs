//! StorageAdapter trait and domain types.
//!
//! # BRD note
//! The brief placed this trait in `crates/kaya-storage`, but it lives here in
//! `kaya-core` to avoid a circular dependency: `commit_edit` (in `kaya-core`)
//! takes `Arc<dyn StorageAdapter>`, so the trait must be in a crate that neither
//! `kaya-storage` nor `kaya-core` imports. Moving it here keeps the dependency
//! graph acyclic. TODO: flag in BRD §8 revision.

use async_trait::async_trait;
use uuid::Uuid;

// ── Domain types ──────────────────────────────────────────────────────────────

/// A knowledge-base document.
///
/// Frontmatter fields follow FR-1 / FR-2 from the BRD. The `body` field holds
/// the raw Markdown text that follows the YAML frontmatter block. The `path`
/// field is the path of the `.md` file relative to the content directory; it is
/// `None` for in-memory / not-yet-persisted documents.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Document {
    /// Stable UUID written into frontmatter (FR-2). Never changes across renames.
    pub id: Uuid,
    /// Document title (frontmatter `title`, required).
    pub title: String,
    /// Optional owner (frontmatter `owner`).
    pub owner: Option<String>,
    /// Optional ISO date of last review (frontmatter `last_reviewed`).
    pub last_reviewed: Option<chrono::NaiveDate>,
    /// Tag list (frontmatter `tags`).
    pub tags: Vec<String>,
    /// UUIDs of related documents (frontmatter `related_docs`).
    pub related_docs: Vec<Uuid>,
    /// Raw Markdown body (everything after the closing `---` delimiter).
    pub body: String,
    /// Path of the `.md` file relative to the content directory, if known.
    #[serde(skip)]
    pub path: Option<std::path::PathBuf>,
}

/// A paragraph chunk extracted from a document body.
///
/// The `paragraph_id` is derived from `SHA-256(ordinal_bytes | content)` and
/// is stable across re-indexing runs as long as neither the paragraph's
/// position nor its content changes (FR-6).
#[derive(Debug, Clone)]
pub struct Chunk {
    pub document_id: Uuid,
    /// Stable ID: first 16 hex chars of `SHA-256(ordinal_le | content_utf8)`.
    pub paragraph_id: String,
    pub content: String,
    pub ordinal: u32,
}

/// A chunk returned from a text or vector search, ready for citation (FR-8).
#[derive(Debug, Clone)]
pub struct ChunkHit {
    pub document_id: Uuid,
    pub paragraph_id: String,
    pub content: String,
    pub ordinal: u32,
}

/// A vector embedding for a single chunk of a document.
#[derive(Debug, Clone)]
pub struct Embedding {
    pub document_id: Uuid,
    /// Stable paragraph identifier matching [`Chunk::paragraph_id`].
    pub paragraph_id: String,
    pub vector: Vec<f32>,
}

// ── Error ─────────────────────────────────────────────────────────────────────

/// Error type for storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The requested document does not exist.
    #[error("document not found: {0}")]
    NotFound(Uuid),

    /// An underlying I/O or database error.
    #[error("backend error: {0}")]
    Backend(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Abstracts over SQLite (OSS) and Postgres (cloud) storage backends.
///
/// The trait is object-safe: all methods take `&self` and return boxed futures
/// via `async_trait`. Implementations must be `Send + Sync`.
///
/// Two implementations are planned:
/// - `SqliteAdapter` in `crates/kaya-storage` (Apache 2.0)
/// - `PostgresAdapter` in `crates/ee/kaya-postgres-storage` (BSL 1.1)
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    // ── Documents ─────────────────────────────────────────────────────────────

    /// Retrieve a document by its ID. Always reads from disk in OSS mode.
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError>;

    /// Persist a document, inserting or replacing by ID.
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError>;

    /// Remove a document by ID. No-op if the document does not exist.
    async fn delete_document(&self, id: Uuid) -> Result<(), StorageError>;

    /// Return all non-deleted documents.
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError>;

    // ── Chunks and text index ─────────────────────────────────────────────────

    /// Store a chunk in the metadata table and the FTS5 full-text index.
    async fn save_chunk(&self, chunk: &Chunk) -> Result<(), StorageError>;

    /// Delete all chunks (metadata + FTS5 rows) for a document.
    ///
    /// Called before re-indexing a document so the FTS5 table stays
    /// consistent. Embeddings are managed separately via
    /// [`delete_embeddings_for_paragraphs`](Self::delete_embeddings_for_paragraphs).
    async fn delete_chunks_for_document(&self, document_id: Uuid) -> Result<(), StorageError>;

    /// Return `(paragraph_id, content_hash)` pairs for all stored chunks of a
    /// document. Used by [`retrieval::index_document_chunks`] to detect which
    /// paragraphs have changed and must be re-embedded (FR-6).
    async fn get_chunk_hashes(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<(String, String)>, StorageError>;

    /// BM25 full-text search over chunks via SQLite FTS5 (FR-7).
    ///
    /// `query` is passed directly to FTS5; callers should avoid FTS5 special
    /// characters (`*`, `"`, `^`, `:`) or escape them before calling.
    async fn search_text(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ChunkHit>, StorageError>;

    // ── Embeddings ────────────────────────────────────────────────────────────

    /// Persist a vector embedding for a chunk, replacing any existing row.
    async fn save_embeddings(&self, embedding: &Embedding) -> Result<(), StorageError>;

    /// Delete embeddings for specific (document_id, paragraph_id) pairs.
    ///
    /// Called when paragraphs are edited or removed so stale vectors do not
    /// pollute the vector index (FR-6).
    async fn delete_embeddings_for_paragraphs(
        &self,
        document_id: Uuid,
        paragraph_ids: &[String],
    ) -> Result<(), StorageError>;

    /// Vector search: find the `limit` nearest chunks to `query` by cosine
    /// similarity and return them ranked best-first (FR-7).
    ///
    /// In OSS mode this loads all embeddings and computes cosine similarity in
    /// Rust. The cloud mode replaces this with a sqlite-vec / pgvector query.
    async fn search_embeddings(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<ChunkHit>, StorageError>;
}
