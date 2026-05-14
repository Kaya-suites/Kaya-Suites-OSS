// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Per-user monthly spend cap enforcement (FR-35).
//!
//! The check is a pre-invocation gate: the agent layer calls
//! `check_spend_cap` before every agent invocation.  On breach the agent
//! returns a clear throttle error; it does not silently continue.
//!
//! Alerts are sent at 80 % (soft) and 100 % (hard) via Resend.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::MeteringError;
use crate::events::current_period_start;

/// Check whether `user_id` has reached their monthly spend cap.
///
/// The period is the current calendar month.  Returns
/// `MeteringError::SpendCapReached` if the cap has been hit.
pub async fn check_spend_cap(
    pool: &PgPool,
    user_id: Uuid,
    cap_usd: f64,
) -> Result<(), MeteringError> {
    let period_start: DateTime<Utc> = current_period_start()
        .and_hms_opt(0, 0, 0)
        .expect("valid hms")
        .and_utc();

    let spent: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(cost_usd), 0.0)::float8
         FROM usage_events
         WHERE user_id = $1 AND recorded_at >= $2",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_one(pool)
    .await?;

    if spent >= cap_usd {
        return Err(MeteringError::SpendCapReached {
            spent_usd: spent,
            cap_usd,
        });
    }

    Ok(())
}

/// Returns the current-period spend for a user without enforcing the cap.
/// Used for dashboard display and alert thresholds.
pub async fn current_period_spend(pool: &PgPool, user_id: Uuid) -> Result<f64, MeteringError> {
    let period_start: DateTime<Utc> = current_period_start()
        .and_hms_opt(0, 0, 0)
        .expect("valid hms")
        .and_utc();

    let spent: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(cost_usd), 0.0)::float8
         FROM usage_events
         WHERE user_id = $1 AND recorded_at >= $2",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_one(pool)
    .await?;

    Ok(spent)
}
