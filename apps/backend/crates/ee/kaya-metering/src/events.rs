// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Persist a single LLM token-usage record to `usage_events` and roll it up
//! into the monthly `usage_counters` aggregate.

use chrono::{Datelike, NaiveDate, Utc};
use kaya_core::{OperationType, TokenUsage};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::MeteringError;
use crate::pricing::PricingConfig;

/// Map an [`OperationType`] to its snake_case DB string.
pub fn operation_to_str(op: &OperationType) -> &'static str {
    match op {
        OperationType::RetrievalClassification => "retrieval_classification",
        OperationType::DocumentGeneration => "document_generation",
        OperationType::EditProposal => "edit_proposal",
        OperationType::StaleDetection => "stale_detection",
        OperationType::Embedding => "embedding",
    }
}

/// Returns true for operations that count against the D-12 agent invocation quota.
pub fn is_agent_invocation(op: &OperationType) -> bool {
    matches!(
        op,
        OperationType::EditProposal | OperationType::DocumentGeneration
    )
}

/// First day of the current calendar month — used as the period key.
pub fn current_period_start() -> NaiveDate {
    let now = Utc::now();
    NaiveDate::from_ymd_opt(now.year(), now.month(), 1).expect("valid date")
}

/// Persist one LLM call to `usage_events` and roll it up into `usage_counters`.
pub async fn persist_event(
    pool: &PgPool,
    pricing: &PricingConfig,
    user_id: Uuid,
    usage: &TokenUsage,
) -> Result<(), MeteringError> {
    let cost = pricing.compute_cost(&usage.model, usage.input_tokens, usage.output_tokens);
    let op_str = operation_to_str(&usage.operation);
    let is_invocation = is_agent_invocation(&usage.operation);

    sqlx::query(
        "INSERT INTO usage_events (user_id, operation, model, input_tokens, output_tokens, cost_usd)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(op_str)
    .bind(&usage.model)
    .bind(usage.input_tokens as i32)
    .bind(usage.output_tokens as i32)
    .bind(cost)
    .execute(pool)
    .await?;

    let period_start = current_period_start();
    let invocation_delta: i64 = if is_invocation { 1 } else { 0 };

    sqlx::query(
        "INSERT INTO usage_counters
           (user_id, period_start, tokens_in, tokens_out, agent_invocations)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, period_start) DO UPDATE
           SET tokens_in          = usage_counters.tokens_in + EXCLUDED.tokens_in,
               tokens_out         = usage_counters.tokens_out + EXCLUDED.tokens_out,
               agent_invocations  = usage_counters.agent_invocations + EXCLUDED.agent_invocations",
    )
    .bind(user_id)
    .bind(period_start)
    .bind(usage.input_tokens as i64)
    .bind(usage.output_tokens as i64)
    .bind(invocation_delta)
    .execute(pool)
    .await?;

    Ok(())
}
