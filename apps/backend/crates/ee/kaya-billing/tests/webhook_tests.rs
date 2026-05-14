// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Webhook signature unit tests + subscription lifecycle integration tests.
//!
//! # Running integration tests
//!
//! Set `PG_TEST_DATABASE_URL` to a Postgres connection string with the
//! kaya-postgres-storage migrations already applied:
//!
//! ```bash
//! PG_TEST_DATABASE_URL=postgres://... cargo test -p kaya-billing
//! ```
//!
//! Integration tests are skipped when the env var is absent.

use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use kaya_billing::{BillingError, verify_webhook_signature};
use kaya_billing::webhook::{BillingService, PaddleWebhookPayload};
use sha2::Sha256;
use sqlx::Row as _;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn make_signature(secret: &str, ts: i64, body: &[u8]) -> String {
    let msg = format!("{ts}:{}", String::from_utf8_lossy(body));
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn valid_header(secret: &str, body: &[u8]) -> String {
    let ts = now_ts();
    let h1 = make_signature(secret, ts, body);
    format!("ts={ts};h1={h1}")
}

// ── Signature unit tests (no DB required) ────────────────────────────────────

#[test]
fn valid_signature_is_accepted() {
    let body = b"test_payload";
    let header = valid_header("my_secret", body);
    assert!(verify_webhook_signature(&header, body, "my_secret", 300).is_ok());
}

#[test]
fn wrong_secret_is_rejected() {
    let body = b"test_payload";
    let header = valid_header("correct_secret", body);
    assert!(matches!(
        verify_webhook_signature(&header, body, "wrong_secret", 300),
        Err(BillingError::InvalidSignature)
    ));
}

#[test]
fn tampered_body_is_rejected() {
    let original_body = b"original";
    let header = valid_header("secret", original_body);
    assert!(matches!(
        verify_webhook_signature(&header, b"tampered", "secret", 300),
        Err(BillingError::InvalidSignature)
    ));
}

#[test]
fn expired_timestamp_is_rejected() {
    let body = b"body";
    let old_ts: i64 = 1_000; // Unix epoch + 1000 s — ancient
    let h1 = make_signature("secret", old_ts, body);
    let header = format!("ts={old_ts};h1={h1}");
    assert!(matches!(
        verify_webhook_signature(&header, body, "secret", 300),
        Err(BillingError::InvalidSignature)
    ));
}

#[test]
fn malformed_header_is_rejected() {
    assert!(matches!(
        verify_webhook_signature("not_a_valid_header", b"body", "secret", 300),
        Err(BillingError::MalformedSignatureHeader)
    ));
    assert!(matches!(
        verify_webhook_signature("ts=123", b"body", "secret", 300),
        Err(BillingError::MalformedSignatureHeader)
    ));
}

// ── Integration tests (require PG_TEST_DATABASE_URL) ─────────────────────────

async fn test_pool() -> Option<sqlx::PgPool> {
    let url = std::env::var("PG_TEST_DATABASE_URL").ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    Some(pool)
}

fn dummy_billing_svc(pool: sqlx::PgPool) -> BillingService {
    BillingService::new(
        pool,
        "paddle_api_key_placeholder",
        "https://sandbox-api.paddle.com",
        "webhook_secret",
    )
}

fn sub_created_payload(paddle_sub_id: &str, user_id: Uuid) -> PaddleWebhookPayload {
    serde_json::from_value(serde_json::json!({
        "event_type": "subscription.created",
        "notification_id": format!("ntf_{paddle_sub_id}"),
        "data": {
            "id": paddle_sub_id,
            "customer_id": format!("ctm_{}", &paddle_sub_id[..8]),
            "status": "active",
            "current_billing_period": {
                "starts_at": "2024-01-01T00:00:00Z",
                "ends_at": "2024-02-01T00:00:00Z"
            },
            "custom_data": { "user_id": user_id.to_string() }
        }
    }))
    .unwrap()
}

fn sub_cancelled_payload(paddle_sub_id: &str) -> PaddleWebhookPayload {
    serde_json::from_value(serde_json::json!({
        "event_type": "subscription.canceled",
        "notification_id": format!("ntf_cancel_{paddle_sub_id}"),
        "data": {
            "id": paddle_sub_id,
            "customer_id": format!("ctm_{}", &paddle_sub_id[..8]),
            "status": "canceled",
            "current_billing_period": null,
            "custom_data": null
        }
    }))
    .unwrap()
}

fn tx_failed_payload(paddle_sub_id: &str) -> PaddleWebhookPayload {
    serde_json::from_value(serde_json::json!({
        "event_type": "transaction.failed",
        "notification_id": "ntf_txfail_001",
        "data": {
            "id": "txn_test_001",
            "subscription_id": paddle_sub_id,
            "status": "failed"
        }
    }))
    .unwrap()
}

async fn insert_test_user(pool: &sqlx::PgPool, email: &str) -> Uuid {
    sqlx::query("INSERT INTO users (email) VALUES ($1) ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email RETURNING id")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get("id")
        .unwrap()
}

async fn subscription_status(pool: &sqlx::PgPool, paddle_sub_id: &str) -> Option<String> {
    use sqlx::Row as _;
    sqlx::query("SELECT status FROM subscriptions WHERE paddle_subscription_id = $1")
        .bind(paddle_sub_id)
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(|r| r.try_get("status").unwrap())
}

#[tokio::test]
async fn test_subscription_lifecycle() {
    let Some(pool) = test_pool().await else { return };
    let svc = dummy_billing_svc(pool.clone());

    let sub_id = format!("sub_lifecycle_{}", Uuid::new_v4().simple());
    let email = format!("lifecycle_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_test_user(&pool, &email).await;

    // subscription.created
    svc.handle_event(&sub_created_payload(&sub_id, user_id))
        .await
        .unwrap();
    assert_eq!(subscription_status(&pool, &sub_id).await.as_deref(), Some("active"));

    // transaction.failed → grace_period
    svc.handle_event(&tx_failed_payload(&sub_id))
        .await
        .unwrap();
    assert_eq!(subscription_status(&pool, &sub_id).await.as_deref(), Some("grace_period"));

    // subscription.canceled → cancelled
    svc.handle_event(&sub_cancelled_payload(&sub_id))
        .await
        .unwrap();
    assert_eq!(subscription_status(&pool, &sub_id).await.as_deref(), Some("cancelled"));
}

#[tokio::test]
async fn test_webhook_idempotency() {
    let Some(pool) = test_pool().await else { return };
    let svc = dummy_billing_svc(pool.clone());

    let sub_id = format!("sub_idem_{}", Uuid::new_v4().simple());
    let email = format!("idem_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_test_user(&pool, &email).await;

    let payload = sub_created_payload(&sub_id, user_id);

    // First delivery
    svc.handle_event(&payload).await.unwrap();
    // Replayed delivery — must not duplicate the row or change state
    svc.handle_event(&payload).await.unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subscriptions WHERE paddle_subscription_id = $1",
    )
    .bind(&sub_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count, 1, "replayed subscription.created must not create a duplicate row");
}

#[tokio::test]
async fn test_cancelled_state_is_terminal() {
    let Some(pool) = test_pool().await else { return };
    let svc = dummy_billing_svc(pool.clone());

    let sub_id = format!("sub_terminal_{}", Uuid::new_v4().simple());
    let email = format!("terminal_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_test_user(&pool, &email).await;

    svc.handle_event(&sub_created_payload(&sub_id, user_id))
        .await
        .unwrap();
    svc.handle_event(&sub_cancelled_payload(&sub_id))
        .await
        .unwrap();

    // Replaying a transaction.failed after cancellation must not move status.
    svc.handle_event(&tx_failed_payload(&sub_id))
        .await
        .unwrap();

    assert_eq!(
        subscription_status(&pool, &sub_id).await.as_deref(),
        Some("cancelled"),
        "terminal 'cancelled' must not be overwritten by later events"
    );
}

#[tokio::test]
async fn test_refund_window_enforcement() {
    let Some(pool) = test_pool().await else { return };
    let svc = dummy_billing_svc(pool.clone());

    let sub_id = format!("sub_refund_{}", Uuid::new_v4().simple());
    let email = format!("refund_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_test_user(&pool, &email).await;

    svc.handle_event(&sub_created_payload(&sub_id, user_id))
        .await
        .unwrap();

    // Backdating the subscription beyond the 30-day window should trigger an error.
    sqlx::query(
        "UPDATE subscriptions
         SET created_at = now() - INTERVAL '31 days'
         WHERE paddle_subscription_id = $1",
    )
    .bind(&sub_id)
    .execute(&pool)
    .await
    .unwrap();

    let result = svc.request_refund(user_id).await;
    assert!(
        matches!(result, Err(BillingError::RefundWindowClosed)),
        "refund outside 30-day window must be rejected: {result:?}"
    );
}
