// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MeteringError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error(
        "monthly spend cap reached: ${spent_usd:.4} of ${cap_usd:.2} used — agent throttled"
    )]
    SpendCapReached { spent_usd: f64, cap_usd: f64 },

    #[error("{window} token rate limit exceeded: {used} of {limit} tokens used")]
    RateLimitExceeded {
        window: &'static str,
        used: i64,
        limit: i64,
    },

    #[error(
        "global circuit breaker open: aggregate daily spend ${daily_usd:.2} \
         exceeds threshold ${threshold_usd:.2}"
    )]
    CircuitBreakerOpen {
        daily_usd: f64,
        threshold_usd: f64,
    },

    #[error("Paddle API error: {0}")]
    PaddleApi(String),

    #[error("pricing config error: {0}")]
    Config(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
