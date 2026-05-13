// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! axum-login backend and `CloudAuthAdapter`.
//!
//! # Type map
//!
//! ```text
//! AuthUser          — the value stored in the session cookie (implements axum_login::AuthUser)
//! KayaAuthBackend   — implements axum_login::AuthnBackend; mounted as a tower layer
//! CloudAuthAdapter  — per-request wrapper around AuthSession<KayaAuthBackend>;
//!                     implements kaya_core::AuthAdapter for the application layer
//! ```

use async_trait::async_trait;
use axum_login::{AuthSession, AuthUser as AxumAuthUser};
use kaya_core::{AuthAdapter, KayaError, UserSession};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::AuthError;
use crate::magic_link::MagicLinkService;

// ── AuthUser ──────────────────────────────────────────────────────────────────

/// The authenticated user stored in the tower-sessions session store.
///
/// Must be `Serialize + DeserializeOwned` because tower-sessions serialises it
/// to JSON. Must be `Clone` because axum-login holds it behind an `Arc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
}

impl AxumAuthUser for AuthUser {
    type Id = Uuid;

    fn id(&self) -> Self::Id {
        self.id
    }

    /// Changing a user's email invalidates all existing sessions automatically
    /// because axum-login compares this hash on every request.
    fn session_auth_hash(&self) -> &[u8] {
        self.email.as_bytes()
    }
}

// ── KayaAuthBackend ───────────────────────────────────────────────────────────

/// axum-login backend wired into the tower layer stack.
///
/// `authenticate` is the credential-based path (magic-link token → user).
/// `get_user` is the session-restore path (user_id → user, called on every request).
#[derive(Clone)]
pub struct KayaAuthBackend {
    pool: PgPool,
    magic_link_svc: std::sync::Arc<MagicLinkService>,
}

impl KayaAuthBackend {
    pub fn new(pool: PgPool, magic_link_svc: std::sync::Arc<MagicLinkService>) -> Self {
        Self { pool, magic_link_svc }
    }
}

/// Credential type used when logging in via a magic-link token.
#[derive(Debug, Clone, Deserialize)]
pub struct MagicLinkCredentials {
    pub token: String,
}

#[async_trait]
impl axum_login::AuthnBackend for KayaAuthBackend {
    type User = AuthUser;
    type Credentials = MagicLinkCredentials;
    type Error = AuthError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        match self.magic_link_svc.verify(&creds.token).await {
            Ok((user_id, email)) => Ok(Some(AuthUser { id: user_id, email })),
            Err(crate::error::MagicLinkError::Invalid)
            | Err(crate::error::MagicLinkError::Expired)
            | Err(crate::error::MagicLinkError::AlreadyUsed) => Ok(None),
            Err(e) => Err(AuthError::from(e)),
        }
    }

    async fn get_user(
        &self,
        user_id: &axum_login::UserId<Self>,
    ) -> Result<Option<Self::User>, Self::Error> {
        let row = sqlx::query("SELECT id, email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| AuthUser {
            id: r.try_get("id").unwrap(),
            email: r.try_get("email").unwrap(),
        }))
    }
}

// ── CloudAuthAdapter ──────────────────────────────────────────────────────────

/// Per-request wrapper around `AuthSession<KayaAuthBackend>` that implements
/// the `kaya_core::AuthAdapter` trait consumed by business-logic handlers.
///
/// Construct in an axum handler by extracting both the `AuthSession` and then
/// calling `CloudAuthAdapter::new(auth_session)`.
pub struct CloudAuthAdapter {
    session: AuthSession<KayaAuthBackend>,
}

impl CloudAuthAdapter {
    pub fn new(session: AuthSession<KayaAuthBackend>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl AuthAdapter for CloudAuthAdapter {
    async fn current_user(&self) -> Result<Option<UserSession>, KayaError> {
        Ok(self.session.user.as_ref().map(|u| UserSession { user_id: u.id }))
    }

    async fn require_auth(&self) -> Result<UserSession, KayaError> {
        self.current_user()
            .await?
            .ok_or(KayaError::Unauthenticated)
    }
}
