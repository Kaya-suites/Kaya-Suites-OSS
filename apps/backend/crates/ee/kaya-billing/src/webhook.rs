// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Paddle webhook signature verification and idempotent event processing.
//!
//! # Signature verification
//!
//! Paddle sends `Paddle-Signature: ts=<unix_epoch>;h1=<hex>`.
//! The signed payload is `"{ts}:{raw_body}"` HMAC-SHA256'd with the webhook
//! secret configured in Paddle's Notifications dashboard.
//! Timestamps older than `max_age_secs` are rejected to prevent replays.
//!
//! # Paddle ID mapping
//!
//! | Paddle field                         | DB column                              |
//! |--------------------------------------|----------------------------------------|
//! | `data.id` (subscription events)      | `subscriptions.paddle_subscription_id` |
//! | `data.customer_id`                   | `subscriptions.paddle_customer_id`     |
//! | `data.subscription_id` (tx events)   | `subscriptions.paddle_subscription_id` |
//! | `data.custom_data.user_id`           | `subscriptions.user_id`                |
//!
//! # Idempotency
//!
//! `subscription.created` checks for an existing row before inserting and uses
//! `ON CONFLICT (user_id) DO UPDATE` so re-subscribes after cancellation are
//! handled gracefully.  All update handlers include `AND status NOT IN
//! ('cancelled', 'refunded')` guards so terminal states are never overwritten
//! by a replayed event.

use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::{BillingError, SubscriptionStatus};

type HmacSha256 = Hmac<Sha256>;

// ── Signature verification ────────────────────────────────────────────────────

/// Verify the `Paddle-Signature` header against the raw request body.
///
/// `max_age_secs` limits the accepted clock skew / replay window (use 300 in
/// production).
pub fn verify_webhook_signature(
    header: &str,
    body: &[u8],
    secret: &str,
    max_age_secs: i64,
) -> Result<(), BillingError> {
    let (ts_str, h1_hex) = parse_signature_header(header)?;

    let ts: i64 = ts_str
        .parse()
        .map_err(|_| BillingError::MalformedSignatureHeader)?;

    let age = Utc::now().timestamp() - ts;
    if age.abs() > max_age_secs {
        warn!(ts = ts, age_secs = age, "Paddle webhook timestamp out of window");
        return Err(BillingError::InvalidSignature);
    }

    let signed_payload = format!("{ts}:{}", String::from_utf8_lossy(body));

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    if !constant_time_eq(&expected, h1_hex) {
        warn!("Paddle webhook HMAC mismatch");
        return Err(BillingError::InvalidSignature);
    }

    Ok(())
}

fn parse_signature_header(header: &str) -> Result<(&str, &str), BillingError> {
    let mut ts = None;
    let mut h1 = None;
    for part in header.split(';') {
        if let Some(v) = part.strip_prefix("ts=") {
            ts = Some(v);
        } else if let Some(v) = part.strip_prefix("h1=") {
            h1 = Some(v);
        }
    }
    match (ts, h1) {
        (Some(t), Some(h)) => Ok((t, h)),
        _ => Err(BillingError::MalformedSignatureHeader),
    }
}

/// Constant-time string comparison to prevent timing side-channels.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// ── Webhook payload types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PaddleWebhookPayload {
    pub event_type: String,
    pub notification_id: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct BillingPeriod {
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct SubscriptionData {
    id: String,
    customer_id: String,
    status: String,
    current_billing_period: Option<BillingPeriod>,
    custom_data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TransactionData {
    id: String,
    subscription_id: Option<String>,
}

// ── BillingService ────────────────────────────────────────────────────────────

/// Business-logic service for Paddle billing.
///
/// All event handlers are idempotent.  Hold behind an `Arc` and clone freely
/// into Axum handlers.
#[derive(Clone)]
pub struct BillingService {
    pool: PgPool,
    /// Paddle API key for REST calls (cancellation, refunds).
    paddle_api_key: String,
    /// Paddle REST API base URL.
    /// Sandbox: `https://sandbox-api.paddle.com`
    /// Live:    `https://api.paddle.com`
    paddle_api_base: String,
    /// Webhook signing secret from Paddle Notifications dashboard.
    pub webhook_secret: String,
    http: reqwest::Client,
}

impl BillingService {
    pub fn new(
        pool: PgPool,
        paddle_api_key: impl Into<String>,
        paddle_api_base: impl Into<String>,
        webhook_secret: impl Into<String>,
    ) -> Self {
        Self {
            pool,
            paddle_api_key: paddle_api_key.into(),
            paddle_api_base: paddle_api_base.into(),
            webhook_secret: webhook_secret.into(),
            http: reqwest::Client::new(),
        }
    }

    /// Verify the `Paddle-Signature` header for an inbound webhook request.
    pub fn verify_webhook(&self, header: &str, body: &[u8]) -> Result<(), BillingError> {
        verify_webhook_signature(header, body, &self.webhook_secret, 300)
    }

    /// Dispatch a verified webhook payload to the appropriate handler.
    pub async fn handle_event(&self, payload: &PaddleWebhookPayload) -> Result<(), BillingError> {
        info!(
            event_type = %payload.event_type,
            notification_id = %payload.notification_id,
            "processing Paddle webhook"
        );
        match payload.event_type.as_str() {
            "subscription.created"  => self.on_subscription_created(payload).await,
            "subscription.updated"  => self.on_subscription_updated(payload).await,
            "subscription.past_due" => self.on_subscription_past_due(payload).await,
            "subscription.canceled" => self.on_subscription_cancelled(payload).await,
            "transaction.completed" => self.on_transaction_completed(payload).await,
            "transaction.failed"    => self.on_transaction_failed(payload).await,
            other => {
                info!(event_type = %other, "ignoring unhandled Paddle event");
                Ok(())
            }
        }
    }

    // ── Subscription handlers ─────────────────────────────────────────────────

    async fn on_subscription_created(
        &self,
        payload: &PaddleWebhookPayload,
    ) -> Result<(), BillingError> {
        let sub: SubscriptionData = serde_json::from_value(payload.data.clone())?;

        let user_id = sub
            .custom_data
            .as_ref()
            .and_then(|d| d.get("user_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let Some(user_id) = user_id else {
            warn!(
                paddle_sub_id = %sub.id,
                "subscription.created: missing user_id in custom_data — cannot provision"
            );
            return Ok(());
        };

        // Idempotency: if this exact Paddle subscription ID is already in the DB, skip.
        let already_exists = sqlx::query(
            "SELECT 1 FROM subscriptions WHERE paddle_subscription_id = $1",
        )
        .bind(&sub.id)
        .fetch_optional(&self.pool)
        .await?
        .is_some();

        if already_exists {
            info!(paddle_sub_id = %sub.id, "subscription.created: already provisioned, skipping");
            return Ok(());
        }

        let period = sub.current_billing_period.as_ref();
        let status = SubscriptionStatus::from_paddle_status(&sub.status);

        // ON CONFLICT (user_id) handles re-subscriptions: a user cancels then
        // resubscribes, generating a new Paddle subscription ID.
        sqlx::query(
            "INSERT INTO subscriptions
               (user_id, paddle_subscription_id, paddle_customer_id, status,
                current_period_start, current_period_end)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id) DO UPDATE
               SET paddle_subscription_id = EXCLUDED.paddle_subscription_id,
                   paddle_customer_id     = EXCLUDED.paddle_customer_id,
                   status                 = EXCLUDED.status,
                   current_period_start   = EXCLUDED.current_period_start,
                   current_period_end     = EXCLUDED.current_period_end,
                   updated_at             = now()",
        )
        .bind(user_id)
        .bind(&sub.id)
        .bind(&sub.customer_id)
        .bind(status.as_str())
        .bind(period.map(|p| p.starts_at))
        .bind(period.map(|p| p.ends_at))
        .execute(&self.pool)
        .await?;

        info!(paddle_sub_id = %sub.id, %user_id, "subscription provisioned");
        Ok(())
    }

    async fn on_subscription_updated(
        &self,
        payload: &PaddleWebhookPayload,
    ) -> Result<(), BillingError> {
        let sub: SubscriptionData = serde_json::from_value(payload.data.clone())?;
        let period = sub.current_billing_period.as_ref();
        let status = SubscriptionStatus::from_paddle_status(&sub.status);

        sqlx::query(
            "UPDATE subscriptions
             SET status               = $2,
                 current_period_start = COALESCE($3, current_period_start),
                 current_period_end   = COALESCE($4, current_period_end),
                 updated_at           = now()
             WHERE paddle_subscription_id = $1
               AND status NOT IN ('cancelled', 'refunded')",
        )
        .bind(&sub.id)
        .bind(status.as_str())
        .bind(period.map(|p| p.starts_at))
        .bind(period.map(|p| p.ends_at))
        .execute(&self.pool)
        .await?;

        info!(paddle_sub_id = %sub.id, new_status = %status.as_str(), "subscription updated");
        Ok(())
    }

    async fn on_subscription_past_due(
        &self,
        payload: &PaddleWebhookPayload,
    ) -> Result<(), BillingError> {
        let sub: SubscriptionData = serde_json::from_value(payload.data.clone())?;

        sqlx::query(
            "UPDATE subscriptions
             SET status = 'grace_period', updated_at = now()
             WHERE paddle_subscription_id = $1
               AND status NOT IN ('cancelled', 'refunded')",
        )
        .bind(&sub.id)
        .execute(&self.pool)
        .await?;

        info!(paddle_sub_id = %sub.id, "subscription past_due → grace_period");
        // TODO(billing): send payment reminder email via Resend
        Ok(())
    }

    async fn on_subscription_cancelled(
        &self,
        payload: &PaddleWebhookPayload,
    ) -> Result<(), BillingError> {
        let sub: SubscriptionData = serde_json::from_value(payload.data.clone())?;

        sqlx::query(
            "UPDATE subscriptions
             SET status = 'cancelled', updated_at = now()
             WHERE paddle_subscription_id = $1
               AND status != 'refunded'",
        )
        .bind(&sub.id)
        .execute(&self.pool)
        .await?;

        info!(paddle_sub_id = %sub.id, "subscription cancelled");
        Ok(())
    }

    // ── Transaction handlers ──────────────────────────────────────────────────

    async fn on_transaction_completed(
        &self,
        payload: &PaddleWebhookPayload,
    ) -> Result<(), BillingError> {
        let tx: TransactionData = serde_json::from_value(payload.data.clone())?;

        if let Some(ref sub_id) = tx.subscription_id {
            sqlx::query(
                "UPDATE subscriptions
                 SET status = 'active', updated_at = now()
                 WHERE paddle_subscription_id = $1
                   AND status NOT IN ('cancelled', 'refunded')",
            )
            .bind(sub_id)
            .execute(&self.pool)
            .await?;

            info!(transaction_id = %tx.id, paddle_sub_id = %sub_id, "transaction completed → active");
        }

        Ok(())
    }

    async fn on_transaction_failed(
        &self,
        payload: &PaddleWebhookPayload,
    ) -> Result<(), BillingError> {
        let tx: TransactionData = serde_json::from_value(payload.data.clone())?;

        if let Some(ref sub_id) = tx.subscription_id {
            sqlx::query(
                "UPDATE subscriptions
                 SET status = 'grace_period', updated_at = now()
                 WHERE paddle_subscription_id = $1
                   AND status NOT IN ('cancelled', 'refunded')",
            )
            .bind(sub_id)
            .execute(&self.pool)
            .await?;

            warn!(transaction_id = %tx.id, paddle_sub_id = %sub_id, "transaction failed → grace_period");
        }

        Ok(())
    }

    // ── 30-day money-back refund ──────────────────────────────────────────────

    /// Issue a 30-day money-back refund for `user_id`.
    ///
    /// Immediately cancels the Paddle subscription and marks the DB row as
    /// `refunded`.  Returns `BillingError::RefundWindowClosed` when the
    /// subscription was created more than 30 days ago.
    pub async fn request_refund(&self, user_id: Uuid) -> Result<(), BillingError> {
        let row = sqlx::query(
            "SELECT id, paddle_subscription_id, created_at
             FROM subscriptions
             WHERE user_id = $1
               AND status NOT IN ('cancelled', 'refunded')
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(BillingError::SubscriptionNotFound)?;

        let sub_db_id: Uuid = row.try_get("id").unwrap();
        let paddle_sub_id: Option<String> = row.try_get("paddle_subscription_id").unwrap_or(None);
        let created_at: DateTime<Utc> = row.try_get("created_at").unwrap();

        if Utc::now() - created_at > Duration::days(30) {
            return Err(BillingError::RefundWindowClosed);
        }

        if let Some(ref pid) = paddle_sub_id {
            self.cancel_paddle_subscription(pid).await?;
        }

        sqlx::query(
            "UPDATE subscriptions SET status = 'refunded', updated_at = now() WHERE id = $1",
        )
        .bind(sub_db_id)
        .execute(&self.pool)
        .await?;

        info!(%user_id, "subscription refunded within 30-day window");
        // TODO(billing): send refund confirmation email via Resend
        Ok(())
    }

    async fn cancel_paddle_subscription(&self, paddle_sub_id: &str) -> Result<(), BillingError> {
        let url = format!(
            "{}/subscriptions/{}/cancel",
            self.paddle_api_base.trim_end_matches('/'),
            paddle_sub_id,
        );

        let resp = self
            .http
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.paddle_api_key),
            )
            .json(&serde_json::json!({"effective_from": "immediately"}))
            .send()
            .await
            .map_err(|e| BillingError::PaddleApi(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(BillingError::PaddleApi(format!("{status}: {text}")));
        }

        Ok(())
    }
}
