// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Subscription lifecycle states.
///
/// | Paddle status          | Kaya status  |
/// |------------------------|--------------|
/// | `active`               | Active       |
/// | `past_due` / `paused`  | GracePeriod  |
/// | `canceled`             | Cancelled    |
/// | *(30-day refund path)* | Refunded     |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    GracePeriod,
    Cancelled,
    Refunded,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::GracePeriod => "grace_period",
            Self::Cancelled => "cancelled",
            Self::Refunded => "refunded",
        }
    }

    pub fn from_paddle_status(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "past_due" | "paused" => Self::GracePeriod,
            "canceled" | "cancelled" => Self::Cancelled,
            _ => Self::Active,
        }
    }
}

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("webhook signature invalid")]
    InvalidSignature,

    #[error("webhook signature header malformed")]
    MalformedSignatureHeader,

    #[error("Paddle API error: {0}")]
    PaddleApi(String),

    #[error("subscription not found")]
    SubscriptionNotFound,

    #[error("refund window closed — subscription is older than 30 days")]
    RefundWindowClosed,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
