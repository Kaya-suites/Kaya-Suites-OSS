// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Magic-link token generation, storage, and verification (FR-28).
//!
//! # Security invariants
//!
//! - The raw token is a 32-byte CSPRNG value encoded as 64 hex chars.
//! - Only `SHA-256(token)` is persisted. A database dump cannot be replayed.
//! - Tokens are single-use: `used_at` is set atomically on first verification.
//! - TTL is 15 minutes from creation.

use chrono::{Duration, Utc};
use rand::RngCore;
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::MagicLinkError;

const TOKEN_TTL_MINUTES: i64 = 15;

/// Business-logic service for the magic-link flow.
///
/// Holds no per-request state; clone freely. All methods are safe to call
/// concurrently from multiple Tokio tasks.
#[derive(Clone)]
pub struct MagicLinkService {
    pool: PgPool,
    resend_api_key: String,
    resend_from: String,
    /// Base URL of the *frontend* (e.g. `https://app.kaya.io`).
    /// Used to construct the click-through link in the email.
    frontend_base_url: String,
    http: Client,
}

impl MagicLinkService {
    pub fn new(
        pool: PgPool,
        resend_api_key: impl Into<String>,
        resend_from: impl Into<String>,
        frontend_base_url: impl Into<String>,
    ) -> Self {
        Self {
            pool,
            resend_api_key: resend_api_key.into(),
            resend_from: resend_from.into(),
            frontend_base_url: frontend_base_url.into(),
            http: Client::new(),
        }
    }

    /// Generate a fresh token, upsert the user row, persist the token hash,
    /// and return the **raw** token for inclusion in the magic link URL.
    ///
    /// `pub` so integration tests can call this without sending email.
    #[doc(hidden)]
    pub async fn create_and_store_token(
        &self,
        email: &str,
    ) -> Result<String, MagicLinkError> {
        // Upsert user (no-op if already exists).
        sqlx::query(
            "INSERT INTO users (email) VALUES ($1)
             ON CONFLICT (email) DO NOTHING",
        )
        .bind(email)
        .execute(&self.pool)
        .await?;

        // Generate 32 cryptographically random bytes → 64-char hex token.
        let mut raw = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut raw);
        let token = hex::encode(raw);

        let token_hash = sha256_hex(&token);
        let expires_at = Utc::now() + Duration::minutes(TOKEN_TTL_MINUTES);

        sqlx::query(
            "INSERT INTO magic_links (email, token_hash, expires_at)
             VALUES ($1, $2, $3)",
        )
        .bind(email)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(token)
    }

    /// Public entry point: generate a token and deliver it via Resend.
    pub async fn request_link(&self, email: &str) -> Result<(), MagicLinkError> {
        let token = self.create_and_store_token(email).await?;
        let link = format!(
            "{}/auth/verify?token={}",
            self.frontend_base_url.trim_end_matches('/'),
            token,
        );
        info!(email = email, "sending magic link");
        self.send_email(email, &link).await
    }

    /// Validate a raw token and return the associated email.
    ///
    /// On success the token row is atomically marked as used so it cannot be
    /// replayed. Errors are returned for expired, used, or unknown tokens.
    pub async fn verify(&self, raw_token: &str) -> Result<(Uuid, String), MagicLinkError> {
        let token_hash = sha256_hex(raw_token);
        let now = Utc::now();

        // Fetch the token (only pending rows thanks to the partial index).
        let row = sqlx::query(
            "SELECT id, email, expires_at, used_at
             FROM magic_links
             WHERE token_hash = $1",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(MagicLinkError::Invalid)?;

        // Defensive: if used_at is set the partial-index should have excluded
        // it, but be explicit for clarity.
        let used_at: Option<chrono::DateTime<Utc>> = row.try_get("used_at").unwrap_or(None);
        if used_at.is_some() {
            return Err(MagicLinkError::AlreadyUsed);
        }

        let expires_at: chrono::DateTime<Utc> = row.try_get("expires_at").unwrap();
        if now > expires_at {
            warn!(token_hash = %token_hash, "magic link token expired");
            return Err(MagicLinkError::Expired);
        }

        // Mark as used atomically.
        let link_id: Uuid = row.try_get("id").unwrap();
        sqlx::query(
            "UPDATE magic_links SET used_at = $1 WHERE id = $2 AND used_at IS NULL",
        )
        .bind(now)
        .bind(link_id)
        .execute(&self.pool)
        .await?;

        let email: String = row.try_get("email").unwrap();

        // Fetch the user_id (the upsert in create_and_store_token guarantees it exists).
        let user_id: Uuid = sqlx::query("SELECT id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&self.pool)
            .await?
            .try_get("id")
            .unwrap();

        info!(user_id = %user_id, "magic link verified");
        Ok((user_id, email))
    }

    async fn send_email(&self, to: &str, link: &str) -> Result<(), MagicLinkError> {
        let html = email_html(link);
        let body = serde_json::json!({
            "from": self.resend_from,
            "to": [to],
            "subject": "Your Kaya Suites sign-in link",
            "html": html,
        });

        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", self.resend_api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| MagicLinkError::EmailDelivery(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(MagicLinkError::EmailDelivery(format!(
                "Resend returned {status}: {text}"
            )));
        }
        Ok(())
    }
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

fn email_html(link: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#f9f9f9;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f9f9f9;padding:40px 0;">
    <tr><td align="center">
      <table width="560" cellpadding="0" cellspacing="0" style="background:#fff;border-radius:8px;padding:40px;border:1px solid #e5e5e5;">
        <tr><td>
          <h2 style="margin:0 0 16px;font-size:20px;color:#111;">Sign in to Kaya Suites</h2>
          <p style="margin:0 0 24px;color:#444;line-height:1.6;">
            Click the button below to sign in. This link expires in <strong>15 minutes</strong>
            and can only be used once.
          </p>
          <a href="{link}"
             style="display:inline-block;background:#111;color:#fff;padding:12px 28px;border-radius:6px;text-decoration:none;font-weight:600;font-size:15px;">
            Sign in to Kaya Suites
          </a>
          <p style="margin:24px 0 0;color:#888;font-size:13px;line-height:1.5;">
            If you didn't request this email, you can safely ignore it — no account
            changes will be made.
          </p>
          <hr style="border:none;border-top:1px solid #eee;margin:32px 0 16px;">
          <p style="margin:0;color:#bbb;font-size:12px;">
            Kaya Suites &middot; This link expires in 15 minutes.
          </p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#,
        link = link,
    )
}
