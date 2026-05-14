// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Monthly usage summaries and admin aggregate stats.

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::MeteringError;
use crate::events::current_period_start;

/// Per-user summary for the current billing period.
#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    pub period_start: NaiveDate,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_usd: f64,
    pub agent_invocations: i64,
}

/// Per-user stats row for the admin dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct UserStats {
    pub user_id: String,
    pub email: String,
    pub monthly_cost_usd: f64,
    pub agent_invocations: i64,
}

/// Aggregate stats for the founder dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct AdminStats {
    pub aggregate_daily_spend_usd: f64,
    pub aggregate_monthly_spend_usd: f64,
    pub circuit_breaker_active: bool,
    pub top_users: Vec<UserStats>,
    pub total_users: i64,
    pub active_subscriptions: i64,
}

/// Fetch the current-period usage summary for one user.
pub async fn monthly_summary(pool: &PgPool, user_id: Uuid) -> Result<UsageSummary, MeteringError> {
    let period_start = current_period_start();

    let row = sqlx::query(
        "SELECT COALESCE(tokens_in, 0)         AS tokens_in,
                COALESCE(tokens_out, 0)        AS tokens_out,
                COALESCE(agent_invocations, 0) AS agent_invocations
         FROM usage_counters
         WHERE user_id = $1 AND period_start = $2",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_optional(pool)
    .await?;

    let (tokens_in, tokens_out, agent_invocations) = row
        .map(|r| {
            (
                r.try_get::<i64, _>("tokens_in").unwrap_or(0),
                r.try_get::<i64, _>("tokens_out").unwrap_or(0),
                r.try_get::<i64, _>("agent_invocations").unwrap_or(0),
            )
        })
        .unwrap_or((0, 0, 0));

    let cost_usd: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(cost_usd), 0.0)::float8
         FROM usage_events
         WHERE user_id = $1
           AND recorded_at >= $2::date::timestamptz",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_one(pool)
    .await?;

    Ok(UsageSummary {
        period_start,
        tokens_in,
        tokens_out,
        cost_usd,
        agent_invocations,
    })
}

/// Aggregate stats for the founder admin dashboard.
pub async fn admin_stats(pool: &PgPool, circuit_active: bool) -> Result<AdminStats, MeteringError> {
    let period_start = current_period_start();

    let daily_spend: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(cost_usd), 0.0)::float8
         FROM usage_events
         WHERE recorded_at >= date_trunc('day', now() AT TIME ZONE 'UTC')",
    )
    .fetch_one(pool)
    .await?;

    let monthly_spend: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(cost_usd), 0.0)::float8
         FROM usage_events
         WHERE recorded_at >= $1::date::timestamptz",
    )
    .bind(period_start)
    .fetch_one(pool)
    .await?;

    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    let active_subscriptions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM subscriptions WHERE status = 'active'",
    )
    .fetch_one(pool)
    .await?;

    let top_rows = sqlx::query(
        "SELECT u.id, u.email,
                COALESCE(SUM(e.cost_usd), 0.0)::float8        AS monthly_cost,
                COALESCE(MAX(uc.agent_invocations), 0)         AS agent_invocations
         FROM users u
         LEFT JOIN usage_events e ON e.user_id = u.id
               AND e.recorded_at >= $1::date::timestamptz
         LEFT JOIN usage_counters uc ON uc.user_id = u.id
               AND uc.period_start = $1
         GROUP BY u.id, u.email
         ORDER BY monthly_cost DESC
         LIMIT 20",
    )
    .bind(period_start)
    .fetch_all(pool)
    .await?;

    let top_users: Vec<UserStats> = top_rows
        .iter()
        .map(|r| UserStats {
            user_id: r.try_get::<Uuid, _>("id").unwrap().to_string(),
            email: r.try_get("email").unwrap(),
            monthly_cost_usd: r.try_get("monthly_cost").unwrap(),
            agent_invocations: r.try_get("agent_invocations").unwrap(),
        })
        .collect();

    Ok(AdminStats {
        aggregate_daily_spend_usd: daily_spend,
        aggregate_monthly_spend_usd: monthly_spend,
        circuit_breaker_active: circuit_active,
        top_users,
        total_users,
        active_subscriptions,
    })
}
