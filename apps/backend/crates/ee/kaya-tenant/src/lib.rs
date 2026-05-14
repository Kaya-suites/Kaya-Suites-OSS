// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Multi-tenant context and magic-link authentication for Kaya Suites cloud.
//!
//! # Crate layout
//!
//! - [`UserContext`] — per-request tenant identifier used by `PostgresAdapter`.
//! - [`magic_link`] — token generation, storage, email delivery (FR-28).
//! - [`auth_adapter`] — axum-login backend + `CloudAuthAdapter`.
//! - [`error`] — `MagicLinkError` and `AuthError`.

use uuid::Uuid;

pub mod auth_adapter;
pub mod error;
pub mod magic_link;

// ── Public re-exports ─────────────────────────────────────────────────────────

pub use auth_adapter::{AuthUser, CloudAuthAdapter, KayaAuthBackend, MagicLinkCredentials};
pub use error::{AuthError, MagicLinkError};
pub use magic_link::MagicLinkService;

// ── Re-export session types used by callers ───────────────────────────────────

pub use axum_login::AuthSession;
pub use tower_sessions::{Expiry, SessionManagerLayer};
pub use tower_sessions_sqlx_store::PostgresStore;

// ── UserContext ───────────────────────────────────────────────────────────────

/// Per-request tenant context passed into `PostgresAdapter`.
///
/// An instance without a pool-scoped `UserContext` cannot exist —
/// `PostgresAdapter::new` takes this by value, enforcing the
/// multi-tenancy seam described in CLAUDE.md NFR §6.3.
#[derive(Debug, Clone)]
pub struct UserContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
}
