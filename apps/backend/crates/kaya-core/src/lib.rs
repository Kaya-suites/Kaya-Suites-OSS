pub mod auth;
pub mod edit;
pub mod error;
pub mod model_router;
pub mod retrieval;
pub mod storage;

pub use auth::{AuthAdapter, UserSession};
pub use edit::{ApprovalToken, ProposedEdit, ProposedEditKind, commit_edit};
pub use error::KayaError;
pub use model_router::{
    ConfigError, LlmProvider, Meter, ModelRouter, OperationType, TokenUsage,
};
pub use retrieval::{RetrievalResult, chunk_document, index_document_chunks, make_paragraph_id, retrieve};
pub use storage::{Chunk, ChunkHit, Document, Embedding, StorageAdapter, StorageError};
