// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Period-end overage reporting via Paddle (FR-33, D-5: zero margin).
//!
//! Called at the end of each billing period to invoice overage agent
//! invocations above the D-12 allotment (50/month).
//!
//! # Paddle integration
//!
//! Overages are billed by creating a one-time Paddle transaction against the
//! customer. This requires:
//! - The customer's `paddle_customer_id` from the subscriptions table.
//! - A Paddle price ID configured for at-cost overage billing
//!   (`PADDLE_OVERAGE_PRICE_ID` env var).
//!
//! Until a Paddle overage price is configured, this function logs the overage
//! amount and returns `Ok(())` without making an API call.

use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::MeteringError;

const COST_PER_OVERAGE_INVOCATION_USD: f64 = 0.10;

/// Report overage invocations for `user_id` at the close of `period_start`.
///
/// # Arguments
///
/// * `included_invocations` — D-12 allotment (50).
/// * `paddle_overage_price_id` — optional Paddle price ID for overage charges.
///   When `None` the overage is logged but not billed (useful during testing).
pub async fn report_period_overage(
    pool: &PgPool,
    http: &reqwest::Client,
    paddle_api_key: &str,
    paddle_api_base: &str,
    user_id: Uuid,
    period_start: NaiveDate,
    included_invocations: i64,
    paddle_overage_price_id: Option<&str>,
) -> Result<(), MeteringError> {
    // Fetch invocation count and paddle_customer_id in one trip.
    let row = sqlx::query(
        "SELECT uc.agent_invocations, s.paddle_customer_id
         FROM usage_counters uc
         JOIN subscriptions s ON s.user_id = uc.user_id
         WHERE uc.user_id = $1 AND uc.period_start = $2
         LIMIT 1",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        info!(%user_id, "no usage data for period — no overage to report");
        return Ok(());
    };

    let invocations: i64 = row.try_get("agent_invocations").unwrap_or(0);
    let paddle_customer_id: Option<String> = row.try_get("paddle_customer_id").unwrap_or(None);

    let overage = (invocations - included_invocations).max(0);

    if overage == 0 {
        info!(%user_id, "no overage for period {period_start}");
        return Ok(());
    }

    let overage_usd = overage as f64 * COST_PER_OVERAGE_INVOCATION_USD;

    info!(
        %user_id,
        invocations,
        included = included_invocations,
        overage,
        overage_usd,
        "overage detected"
    );

    let Some(price_id) = paddle_overage_price_id else {
        warn!(%user_id, overage_usd, "PADDLE_OVERAGE_PRICE_ID not set — logging overage without billing");
        return Ok(());
    };

    let Some(customer_id) = paddle_customer_id else {
        warn!(%user_id, "no paddle_customer_id — cannot bill overage");
        return Ok(());
    };

    // Create a one-time Paddle transaction for the overage.
    let url = format!(
        "{}/transactions",
        paddle_api_base.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "customer_id": customer_id,
        "items": [{
            "price_id": price_id,
            "quantity": overage,
        }],
        "collection_mode": "automatic",
        "custom_data": {
            "user_id": user_id.to_string(),
            "period_start": period_start.to_string(),
            "overage_invocations": overage,
        }
    });

    let resp = http
        .post(&url)
        .header("Authorization", format!("Bearer {paddle_api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| MeteringError::PaddleApi(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(MeteringError::PaddleApi(format!("{status}: {text}")));
    }

    info!(%user_id, overage_usd, "Paddle overage transaction created");
    Ok(())
}
