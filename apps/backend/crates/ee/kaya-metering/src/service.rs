// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! [`MeteringService`] — public façade for all metering operations.
//!
//! # Agent loop wiring
//!
//! The agent layer (not yet built) should call these methods in order:
//!
//! ```text
//! // Before invocation:
//! svc.pre_invocation_check(user_id).await?;
//!
//! // After each LLM call returns:
//! svc.record_usage(user_id, &token_usage).await?;
//! ```
//!
//! `pre_invocation_check` is a single gate that runs all three checks
//! (circuit breaker → spend cap → rate limit) and returns on the first
//! failure.  Any `MeteringError` variant should be surfaced to the user
//! with a clear message.

use std::sync::Arc;

use chrono::NaiveDate;
use kaya_core::TokenUsage;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::aggregation::{AdminStats, UsageSummary};
use crate::caps::check_spend_cap;
use crate::circuit::CircuitBreaker;
use crate::error::MeteringError;
use crate::events::persist_event;
use crate::overage::report_period_overage;
use crate::pricing::PricingConfig;
use crate::rate_limit::{check_rate_limit, record_usage as record_rl_usage};

/// Runtime configuration knobs for the metering service.
#[derive(Debug, Clone)]
pub struct MeteringConfig {
    /// D-14: monthly spend cap per user in USD.
    pub spend_cap_usd: f64,
    /// Alert threshold (0.0–1.0 fraction of `spend_cap_usd`).
    pub alert_threshold: f64,
    /// D-12: included agent invocations per billing period.
    pub included_invocations: i64,
    /// FR-36: hourly token limit per user.
    pub hourly_token_limit: i64,
    /// FR-36: daily token limit per user.
    pub daily_token_limit: i64,
    /// BRD §12.5: aggregate daily spend circuit-breaker threshold in USD.
    pub circuit_threshold_usd: f64,
    /// Paddle API key (for overage transactions).
    pub paddle_api_key: String,
    /// Paddle API base URL (sandbox or live).
    pub paddle_api_base: String,
    /// Optional Paddle price ID for overage charges; None disables billing.
    pub paddle_overage_price_id: Option<String>,
    /// Resend API key for spend alerts and circuit-breaker notifications.
    pub resend_api_key: String,
    pub resend_from: String,
    pub admin_email: String,
}

impl Default for MeteringConfig {
    fn default() -> Self {
        Self {
            spend_cap_usd: 6.00,
            alert_threshold: 0.80,
            included_invocations: 50,
            hourly_token_limit: 100_000,
            daily_token_limit: 500_000,
            circuit_threshold_usd: 50.00,
            paddle_api_key: String::new(),
            paddle_api_base: "https://sandbox-api.paddle.com".into(),
            paddle_overage_price_id: None,
            resend_api_key: String::new(),
            resend_from: String::new(),
            admin_email: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct MeteringService {
    pool: PgPool,
    pricing: Arc<PricingConfig>,
    config: Arc<MeteringConfig>,
    circuit: Arc<CircuitBreaker>,
    http: reqwest::Client,
}

impl MeteringService {
    pub fn new(pool: PgPool, pricing: PricingConfig, config: MeteringConfig) -> Self {
        let circuit = CircuitBreaker::new(config.circuit_threshold_usd);
        Self {
            pool,
            pricing: Arc::new(pricing),
            config: Arc::new(config),
            circuit: Arc::new(circuit),
            http: reqwest::Client::new(),
        }
    }

    // ── Pre-invocation gate ───────────────────────────────────────────────────

    /// Run all pre-invocation checks in priority order:
    ///
    /// 1. Global circuit breaker (BRD §12.5)
    /// 2. Per-user spend cap (FR-35)
    /// 3. Per-user rate limits (FR-36)
    ///
    /// Returns the first error encountered.  On `Ok` the invocation may
    /// proceed.
    pub async fn pre_invocation_check(&self, user_id: Uuid) -> Result<(), MeteringError> {
        self.circuit.check(&self.pool).await?;
        check_spend_cap(&self.pool, user_id, self.config.spend_cap_usd).await?;
        check_rate_limit(
            &self.pool,
            user_id,
            self.config.hourly_token_limit,
            self.config.daily_token_limit,
        )
        .await?;
        Ok(())
    }

    // ── Usage recording ───────────────────────────────────────────────────────

    /// Persist a single LLM call's token usage and increment rate-limit buckets.
    ///
    /// Call once per `TokenUsage` returned by the model router.
    pub async fn record_usage(
        &self,
        user_id: Uuid,
        usage: &TokenUsage,
    ) -> Result<(), MeteringError> {
        persist_event(&self.pool, &self.pricing, user_id, usage).await?;

        let total_tokens = (usage.input_tokens + usage.output_tokens) as i64;
        record_rl_usage(&self.pool, user_id, total_tokens).await?;

        self.maybe_send_spend_alert(user_id).await;

        Ok(())
    }

    // ── Dashboard queries ─────────────────────────────────────────────────────

    pub async fn monthly_summary(&self, user_id: Uuid) -> Result<UsageSummary, MeteringError> {
        crate::aggregation::monthly_summary(&self.pool, user_id).await
    }

    pub async fn admin_stats(&self) -> Result<AdminStats, MeteringError> {
        crate::aggregation::admin_stats(&self.pool, self.circuit.is_tripped()).await
    }

    // ── Period-end billing ────────────────────────────────────────────────────

    pub async fn report_period_overage(
        &self,
        user_id: Uuid,
        period_start: NaiveDate,
    ) -> Result<(), MeteringError> {
        report_period_overage(
            &self.pool,
            &self.http,
            &self.config.paddle_api_key,
            &self.config.paddle_api_base,
            user_id,
            period_start,
            self.config.included_invocations,
            self.config.paddle_overage_price_id.as_deref(),
        )
        .await
    }

    // ── Circuit breaker management ────────────────────────────────────────────

    pub async fn reset_circuit_breaker(&self) {
        self.circuit.reset(&self.pool).await;
    }

    pub fn circuit_breaker_tripped(&self) -> bool {
        self.circuit.is_tripped()
    }

    pub fn spend_cap_usd(&self) -> f64 {
        self.config.spend_cap_usd
    }

    pub fn included_invocations(&self) -> i64 {
        self.config.included_invocations
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    /// Check whether the user has crossed the 80% alert threshold and log it.
    /// A production build would send a Resend email here.
    async fn maybe_send_spend_alert(&self, user_id: Uuid) {
        let Ok(spent) = crate::caps::current_period_spend(&self.pool, user_id).await else {
            return;
        };
        let alert_usd = self.config.spend_cap_usd * self.config.alert_threshold;
        if spent >= alert_usd && spent < self.config.spend_cap_usd {
            info!(
                %user_id,
                spent_usd = spent,
                cap_usd = self.config.spend_cap_usd,
                "spend alert: user at 80% of cap"
            );
            // TODO(metering): send alert email via Resend
        }
    }
}
