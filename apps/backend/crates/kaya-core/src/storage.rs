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

/// A knowledge-base document.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub content: String,
}

/// A vector embedding for a single chunk of a document.
#[derive(Debug, Clone)]
pub struct Embedding {
    pub document_id: Uuid,
    /// Zero-based index of the chunk within the document.
    pub chunk_index: u32,
    pub vector: Vec<f32>,
}

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

/// Abstracts over SQLite (OSS) and Postgres (cloud) storage backends.
///
/// The trait is object-safe: all methods take `&self` and return boxed futures
/// via `async_trait`. Implementations must be `Send + Sync`.
///
/// Two implementations are planned (not yet written):
/// - `SqliteAdapter` in `crates/kaya-storage` (Apache 2.0)
/// - `PostgresAdapter` in `crates/ee/kaya-postgres-storage` (BSL 1.1)
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    /// Retrieve a document by its ID.
    ///
    /// Returns [`StorageError::NotFound`] if the document does not exist.
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError>;

    /// Persist a document, inserting or replacing by ID.
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError>;

    /// Remove a document by ID. No-op if the document does not exist.
    async fn delete_document(&self, id: Uuid) -> Result<(), StorageError>;

    /// Return all documents, unordered.
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError>;

    /// Find the `limit` nearest embeddings to `query` by cosine similarity.
    async fn search_embeddings(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<Embedding>, StorageError>;

    /// Persist an embedding for a document chunk, inserting or replacing.
    async fn save_embeddings(&self, embedding: &Embedding) -> Result<(), StorageError>;
}
