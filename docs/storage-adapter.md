# StorageAdapter

**Trait location:** `crates/kaya-core/src/storage.rs`  
**License:** Apache 2.0

## Why it lives in `kaya-core`

The brief originally placed this trait in `crates/kaya-storage`, but that creates a circular dependency: `commit_edit` (in `kaya-core`) takes `Arc<dyn StorageAdapter>`, so the trait must be in a crate that neither `kaya-storage` nor `kaya-core` imports. Moving it to `kaya-core` keeps the dependency graph acyclic.

## Domain types

### `Document`

A knowledge-base document. Frontmatter fields follow FR-1/FR-2 from the BRD.

| Field | Type | Description |
|---|---|---|
| `id` | `Uuid` | Stable UUID written into frontmatter. Never changes across renames. |
| `title` | `String` | Document title (frontmatter `title`, required). |
| `owner` | `Option<String>` | Optional owner (frontmatter `owner`). |
| `last_reviewed` | `Option<NaiveDate>` | Optional ISO date of last review. |
| `tags` | `Vec<String>` | Tag list. |
| `related_docs` | `Vec<Uuid>` | UUIDs of related documents. |
| `body` | `String` | Raw Markdown body after the closing `---` delimiter. |
| `path` | `Option<PathBuf>` | Path relative to content directory; `None` for in-memory documents. |

### `Chunk`

A paragraph extracted from a document body. The `paragraph_id` is the first 16 hex characters of `SHA-256(ordinal_le | content_utf8)`, making it stable across re-indexing runs as long as neither the paragraph's position nor content changes (FR-6).

### `ChunkHit`

A chunk returned from text or vector search, ready for citation (FR-8).

### `Embedding`

A vector embedding for a single chunk. Matches a `Chunk` by `paragraph_id`.

### `StorageError`

| Variant | Meaning |
|---|---|
| `NotFound(Uuid)` | Requested document does not exist. |
| `Backend(Box<dyn Error>)` | Underlying I/O or database error. |

## Trait methods

```rust
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    // Documents
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError>;
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError>;
    async fn delete_document(&self, id: Uuid) -> Result<(), StorageError>;
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError>;

    // Chunks and text index (FTS5)
    async fn save_chunk(&self, chunk: &Chunk) -> Result<(), StorageError>;
    async fn delete_chunks_for_document(&self, document_id: Uuid) -> Result<(), StorageError>;
    async fn get_chunk_hashes(&self, document_id: Uuid) -> Result<Vec<(String, String)>, StorageError>;
    async fn search_text(&self, query: &str, limit: usize) -> Result<Vec<ChunkHit>, StorageError>;

    // Embeddings
    async fn save_embeddings(&self, embedding: &Embedding) -> Result<(), StorageError>;
    async fn delete_embeddings_for_paragraphs(&self, document_id: Uuid, paragraph_ids: &[String]) -> Result<(), StorageError>;
    async fn search_embeddings(&self, query: &[f32], limit: usize) -> Result<Vec<ChunkHit>, StorageError>;
}
```

## Implementations

### `SqliteAdapter` (Apache 2.0)

**Location:** `crates/kaya-storage/src/sqlite.rs`

- Persists documents as `.md` files in a content directory.
- Maintains an FTS5 table for BM25 full-text search (`search_text`).
- Loads all embeddings into memory and computes cosine similarity in Rust for `search_embeddings`. Suitable for single-user OSS deployments.

### `PostgresAdapter` (BSL 1.1) — not yet implemented

**Location:** `crates/ee/kaya-postgres-storage/`

- Scoped per `UserContext`; no static query methods (multi-tenancy seam).
- Delegates `search_embeddings` to pgvector for server-side ANN.
- See `docs/ee/postgres-storage.md` for details.

## Usage

Business logic accepts `Arc<dyn StorageAdapter>` and never names a concrete type:

```rust
async fn commit_edit(
    storage: Arc<dyn StorageAdapter>,
    token: ApprovalToken,
    edit: Edit,
) -> Result<Document, KayaError> { … }
```

The binary (`kaya-oss` or `kaya-cloud`) constructs the concrete adapter at startup and passes it through the application via dependency injection.
