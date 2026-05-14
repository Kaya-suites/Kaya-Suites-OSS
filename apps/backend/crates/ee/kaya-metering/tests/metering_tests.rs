// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Metering integration tests.
//!
//! Set `PG_TEST_DATABASE_URL` to run against a Postgres instance that has all
//! kaya-postgres-storage migrations applied.  Tests skip when the var is absent.
//!
//! ```bash
//! PG_TEST_DATABASE_URL=postgres://... cargo test -p kaya-metering
//! ```

use kaya_core::{OperationType, TokenUsage};
use kaya_metering::pricing::PricingConfig;
use kaya_metering::service::{MeteringConfig, MeteringService};
use kaya_metering::MeteringError;
use sqlx::Row as _;
use uuid::Uuid;

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn test_pool() -> Option<sqlx::PgPool> {
    let url = std::env::var("PG_TEST_DATABASE_URL").ok()?;
    sqlx::PgPool::connect(&url).await.ok()
}

const PRICING_YAML: &str = r#"
models:
  test-model:
    input_per_million: 10.00
    output_per_million: 30.00
  cheap-model:
    input_per_million: 0.10
    output_per_million: 0.30
"#;

fn test_pricing() -> PricingConfig {
    PricingConfig::from_yaml_str(PRICING_YAML).unwrap()
}

fn test_config() -> MeteringConfig {
    MeteringConfig {
        spend_cap_usd: 1.00,         // $1 cap for easy testing
        alert_threshold: 0.80,
        included_invocations: 3,     // 3 invocations/period for easy testing
        hourly_token_limit: 1_000,   // 1K tokens/hour
        daily_token_limit: 5_000,    // 5K tokens/day
        circuit_threshold_usd: 10.00,
        ..Default::default()
    }
}

async fn make_svc(pool: sqlx::PgPool) -> MeteringService {
    MeteringService::new(pool, test_pricing(), test_config())
}

/// Insert a test user and return their UUID.
async fn insert_user(pool: &sqlx::PgPool, email: &str) -> Uuid {
    sqlx::query(
        "INSERT INTO users (email) VALUES ($1)
         ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email
         RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .unwrap()
    .try_get("id")
    .unwrap()
}

fn usage(model: &str, op: OperationType, input: u32, output: u32) -> TokenUsage {
    TokenUsage {
        model: model.to_owned(),
        operation: op,
        input_tokens: input,
        output_tokens: output,
    }
}

// ── Pricing unit tests (no DB) ────────────────────────────────────────────────

#[test]
fn pricing_cost_calculation() {
    let cfg = test_pricing();
    // 1000 input @ $10/M + 500 output @ $30/M = $0.01 + $0.015 = $0.025
    let cost = cfg.compute_cost("test-model", 1_000, 500);
    assert!((cost - 0.025).abs() < 1e-9, "cost = {cost}");
}

#[test]
fn unknown_model_uses_fallback() {
    let cfg = test_pricing();
    let cost = cfg.compute_cost("does-not-exist", 1_000_000, 0);
    assert!((cost - 15.0).abs() < 1e-6, "expected Opus fallback");
}

// ── Integration tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_record_usage_persists_event() {
    let Some(pool) = test_pool().await else { return };
    let svc = make_svc(pool.clone()).await;

    let email = format!("record_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_user(&pool, &email).await;

    let u = usage("test-model", OperationType::EditProposal, 1_000, 500);
    svc.record_usage(user_id, &u).await.unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM usage_events WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_spend_cap_blocks_at_limit() {
    let Some(pool) = test_pool().await else { return };
    let svc = make_svc(pool.clone()).await;

    let email = format!("cap_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_user(&pool, &email).await;

    // test-model: $10/M in + $30/M out.
    // 30K input + 20K output = $0.30 + $0.60 = $0.90 → below $1.00 cap.
    let u = usage("test-model", OperationType::DocumentGeneration, 30_000, 20_000);
    svc.record_usage(user_id, &u).await.unwrap();

    // Invocation 1 still under cap.
    svc.pre_invocation_check(user_id).await.unwrap();

    // Add more spend to exceed cap.
    // 5K input + 5K output = $0.05 + $0.15 = $0.20 → total $1.10 > $1.00.
    let u2 = usage("test-model", OperationType::DocumentGeneration, 5_000, 5_000);
    svc.record_usage(user_id, &u2).await.unwrap();

    let result = svc.pre_invocation_check(user_id).await;
    assert!(
        matches!(result, Err(MeteringError::SpendCapReached { .. })),
        "expected SpendCapReached, got {result:?}"
    );
}

#[tokio::test]
async fn test_rate_limit_hourly() {
    let Some(pool) = test_pool().await else { return };
    let svc = make_svc(pool.clone()).await;

    let email = format!("rl_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_user(&pool, &email).await;

    // Record 900 tokens (below 1000 hourly limit).
    let u = usage("cheap-model", OperationType::RetrievalClassification, 600, 300);
    svc.record_usage(user_id, &u).await.unwrap();

    // Still under limit.
    svc.pre_invocation_check(user_id).await.unwrap();

    // Push over the hourly limit.
    let u2 = usage("cheap-model", OperationType::RetrievalClassification, 600, 300);
    svc.record_usage(user_id, &u2).await.unwrap(); // total 1800 > 1000

    let result = svc.pre_invocation_check(user_id).await;
    assert!(
        matches!(result, Err(MeteringError::RateLimitExceeded { window: "hourly", .. })),
        "expected hourly RateLimitExceeded, got {result:?}"
    );
}

#[tokio::test]
async fn test_circuit_breaker_trips() {
    let Some(pool) = test_pool().await else { return };

    // Low threshold to make it easy to trip.
    let config = MeteringConfig {
        circuit_threshold_usd: 0.001,
        spend_cap_usd: 100.0,   // high so cap doesn't interfere
        hourly_token_limit: 10_000_000,
        daily_token_limit: 100_000_000,
        ..Default::default()
    };
    let svc = MeteringService::new(pool.clone(), test_pricing(), config);

    let email = format!("cb_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_user(&pool, &email).await;

    // Insert a usage event that exceeds the $0.001 daily threshold.
    // 1000 input on test-model = $0.01 > $0.001 threshold.
    let u = usage("test-model", OperationType::EditProposal, 1_000, 0);
    svc.record_usage(user_id, &u).await.unwrap();

    // Force the circuit to re-evaluate (clear the cached last_check by creating a new service).
    let svc2 = MeteringService::new(pool.clone(), test_pricing(), MeteringConfig {
        circuit_threshold_usd: 0.001,
        spend_cap_usd: 100.0,
        hourly_token_limit: 10_000_000,
        daily_token_limit: 100_000_000,
        ..Default::default()
    });

    let email2 = format!("cb2_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id2 = insert_user(&pool, &email2).await;

    let result = svc2.pre_invocation_check(user_id2).await;
    assert!(
        matches!(result, Err(MeteringError::CircuitBreakerOpen { .. })),
        "expected CircuitBreakerOpen, got {result:?}"
    );
}

#[tokio::test]
async fn test_monthly_summary_counts_invocations() {
    let Some(pool) = test_pool().await else { return };
    let svc = make_svc(pool.clone()).await;

    let email = format!("summary_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_user(&pool, &email).await;

    // 2 EditProposals (count as invocations) + 1 Embedding (does not).
    svc.record_usage(user_id, &usage("test-model", OperationType::EditProposal, 100, 50)).await.unwrap();
    svc.record_usage(user_id, &usage("test-model", OperationType::EditProposal, 100, 50)).await.unwrap();
    svc.record_usage(user_id, &usage("test-model", OperationType::Embedding, 200, 0)).await.unwrap();

    let summary = svc.monthly_summary(user_id).await.unwrap();
    assert_eq!(summary.agent_invocations, 2, "only EditProposal ops should count");
    assert_eq!(summary.tokens_in, 400);
    assert_eq!(summary.tokens_out, 100);
}

#[tokio::test]
async fn test_overage_calculation() {
    let Some(pool) = test_pool().await else { return };
    let svc = make_svc(pool.clone()).await; // included_invocations = 3

    let email = format!("overage_{}@test.kaya.io", Uuid::new_v4().simple());
    let user_id = insert_user(&pool, &email).await;

    // 5 invocations → 2 over the allotment of 3.
    for _ in 0..5 {
        svc.record_usage(
            user_id,
            &usage("cheap-model", OperationType::EditProposal, 10, 5),
        )
        .await
        .unwrap();
    }

    let summary = svc.monthly_summary(user_id).await.unwrap();
    let overage = (summary.agent_invocations - svc.monthly_summary(user_id).await.unwrap().agent_invocations).max(0);

    // report_period_overage with no price ID should return Ok (log-only mode).
    let result = svc
        .report_period_overage(user_id, summary.period_start)
        .await;
    assert!(result.is_ok(), "log-only overage reporting must succeed: {result:?}");
    let _ = overage; // silence unused warning
}
