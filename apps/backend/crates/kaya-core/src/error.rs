use crate::storage::StorageError;

/// Top-level application error.
#[derive(Debug, thiserror::Error)]
pub enum KayaError {
    /// A storage operation failed.
    #[error("storage: {0}")]
    Storage(#[from] StorageError),

    /// The request requires authentication but none was provided.
    #[error("unauthenticated")]
    Unauthenticated,

    /// The authenticated user does not have permission for this action.
    #[error("forbidden")]
    Forbidden,

    /// An unexpected internal error occurred.
    #[error("internal: {0}")]
    Internal(String),
}
