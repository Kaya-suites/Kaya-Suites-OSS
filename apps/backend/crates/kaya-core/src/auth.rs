//! AuthAdapter trait and user-session types.

use async_trait::async_trait;
use uuid::Uuid;
use crate::error::KayaError;

/// An authenticated user session.
#[derive(Debug, Clone)]
pub struct UserSession {
    /// Unique identifier for the authenticated user.
    pub user_id: Uuid,
}

/// Abstracts authentication between the OSS single-user mode and cloud
/// magic-link sessions.
///
/// Two implementations are planned (not yet written):
/// - `LocalAuthAdapter` (Apache 2.0) — returns a fixed single-user session,
///   no network call, used by the OSS binary.
/// - `CloudAuthAdapter` (BSL 1.1) — validates a session cookie against the
///   database, used by the cloud binary.
#[async_trait]
pub trait AuthAdapter: Send + Sync {
    /// Return the current session if the request is authenticated, or `None`.
    async fn current_user(&self) -> Result<Option<UserSession>, KayaError>;

    /// Return the current session, or `Err(KayaError::Unauthenticated)` if
    /// the request carries no valid credentials.
    async fn require_auth(&self) -> Result<UserSession, KayaError>;
}
