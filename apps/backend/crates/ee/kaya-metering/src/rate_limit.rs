// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Hourly and daily token-bucket rate limits (FR-36).
//!
//! # Implementation choice: Postgres (not Redis)
//!
//! v0 uses Postgres `rate_limit_windows` rows as token buckets.  Each row
//! covers one user × one window type × one window_start timestamp.  Upserts
//! with `ON CONFLICT … DO UPDATE` are atomic under Postgres' default READ
//! COMMITTED isolation, which is sufficient for soft rate-limiting.
//!
//! Upgrade path: replace `check_and_record` with Redis INCR + EXPIRE calls
//! when sub-millisecond latency or strict atomicity is required.
//!
//! # Bucket lifecycle
//!
//! The check precedes the LLM call: we record the expected token budget
//! (zero for a pre-check) and verify the current window usage is below the
//! limit.  After the call, `record_usage` increments the bucket by the actual
//! token count.  A user can therefore overshoot by one invocation — acceptable
//! for a soft limit.

use chrono::{DateTime, TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::MeteringError;

fn truncate_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    let ts = dt.timestamp();
    Utc.timestamp_opt(ts - (ts % 3600), 0)
        .single()
        .expect("valid ts")
}

fn truncate_to_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid hms")
        .and_utc()
}

async fn window_usage(
    pool: &PgPool,
    user_id: Uuid,
    window_type: &str,
    window_start: DateTime<Utc>,
) -> Result<i64, MeteringError> {
    let used: i64 = sqlx::query_scalar(
        "SELECT COALESCE(tokens_used, 0)
         FROM rate_limit_windows
         WHERE user_id = $1 AND window_type = $2 AND window_start = $3",
    )
    .bind(user_id)
    .bind(window_type)
    .bind(window_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0i64);

    Ok(used)
}

async fn increment_window(
    pool: &PgPool,
    user_id: Uuid,
    window_type: &str,
    window_start: DateTime<Utc>,
    tokens: i64,
) -> Result<(), MeteringError> {
    sqlx::query(
        "INSERT INTO rate_limit_windows (user_id, window_type, window_start, tokens_used)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (user_id, window_type, window_start)
         DO UPDATE SET tokens_used = rate_limit_windows.tokens_used + EXCLUDED.tokens_used",
    )
    .bind(user_id)
    .bind(window_type)
    .bind(window_start)
    .bind(tokens)
    .execute(pool)
    .await?;
    Ok(())
}

/// Check whether `user_id` is below both rate limits.
///
/// Does NOT modify any counters.  Call `record_usage` after the LLM call
/// completes to increment the buckets.
pub async fn check_rate_limit(
    pool: &PgPool,
    user_id: Uuid,
    hourly_limit: i64,
    daily_limit: i64,
) -> Result<(), MeteringError> {
    let now = Utc::now();
    let hour_start = truncate_to_hour(now);
    let day_start = truncate_to_day(now);

    let hourly = window_usage(pool, user_id, "hourly", hour_start).await?;
    if hourly >= hourly_limit {
        return Err(MeteringError::RateLimitExceeded {
            window: "hourly",
            used: hourly,
            limit: hourly_limit,
        });
    }

    let daily = window_usage(pool, user_id, "daily", day_start).await?;
    if daily >= daily_limit {
        return Err(MeteringError::RateLimitExceeded {
            window: "daily",
            used: daily,
            limit: daily_limit,
        });
    }

    Ok(())
}

/// Increment the hourly and daily buckets by the actual token count consumed.
///
/// Call after a successful LLM call to keep buckets accurate.
pub async fn record_usage(
    pool: &PgPool,
    user_id: Uuid,
    tokens: i64,
) -> Result<(), MeteringError> {
    let now = Utc::now();
    increment_window(pool, user_id, "hourly", truncate_to_hour(now), tokens).await?;
    increment_window(pool, user_id, "daily", truncate_to_day(now), tokens).await?;
    Ok(())
}
