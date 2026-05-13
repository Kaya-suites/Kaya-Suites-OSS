// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1

/// Errors that can occur during magic-link token lifecycle.
#[derive(Debug, thiserror::Error)]
pub enum MagicLinkError {
    #[error("token not found or already used")]
    Invalid,

    #[error("token has expired")]
    Expired,

    #[error("token has already been used")]
    AlreadyUsed,

    #[error("email send failed: {0}")]
    EmailDelivery(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Errors surfaced by the axum-login auth backend.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    MagicLink(#[from] MagicLinkError),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
