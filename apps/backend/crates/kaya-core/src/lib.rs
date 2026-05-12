pub mod auth;
pub mod diff;
pub mod edit;
pub mod error;
pub mod model_router;
pub mod storage;

// agent is declared here; populated in the next step
pub mod agent;

pub use auth::{AuthAdapter, UserSession};
pub use diff::{ParagraphChange, ParagraphDiff};
pub use edit::{ApprovalToken, ProposedEdit, ProposedEditKind, commit_edit};
pub use error::KayaError;
pub use model_router::{
    ConfigError, LlmProvider, Meter, ModelRouter, OperationType, TokenUsage,
};
pub use storage::{Document, Embedding, StorageAdapter, StorageError};
