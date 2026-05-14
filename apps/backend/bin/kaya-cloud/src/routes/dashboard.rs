// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! User-facing dashboard routes.
//!
//! - `GET /billing/status`   — current subscription state + Paddle portal URL
//! - `GET /metering/summary` — current-period usage vs limits

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use kaya_billing::BillingService;
use kaya_metering::MeteringService;
use kaya_tenant::{AuthSession, KayaAuthBackend};
use chrono::Datelike as _;
use serde::Serialize;
use sqlx::PgPool;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/billing/status", get(billing_status))
        .route("/metering/summary", get(metering_summary))
}

// ── GET /billing/status ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct BillingStatus {
    status: String,
    current_period_end: Option<String>,
    /// Days remaining in the 30-day refund window. None if not applicable.
    refund_days_remaining: Option<i64>,
    paddle_customer_id: Option<String>,
}

async fn billing_status(
    State(pool): State<PgPool>,
    State(billing_svc): State<Arc<BillingService>>,
    auth: AuthSession<KayaAuthBackend>,
) -> Response {
    let user = match auth.user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let row = sqlx::query(
        "SELECT status, current_period_end, created_at, paddle_customer_id
         FROM subscriptions
         WHERE user_id = $1
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(user.id)
    .fetch_optional(&pool)
    .await;

    let _ = &billing_svc; // referenced for type extraction

    match row {
        Ok(Some(r)) => {
            use sqlx::Row as _;
            let status: String = r.try_get("status").unwrap_or_else(|_| "unknown".into());
            let current_period_end: Option<chrono::DateTime<chrono::Utc>> =
                r.try_get("current_period_end").unwrap_or(None);
            let created_at: Option<chrono::DateTime<chrono::Utc>> =
                r.try_get("created_at").unwrap_or(None);
            let paddle_customer_id: Option<String> =
                r.try_get("paddle_customer_id").unwrap_or(None);

            let refund_days_remaining = created_at.map(|ca| {
                let window_end = ca + chrono::Duration::days(30);
                let now = chrono::Utc::now();
                let remaining = (window_end - now).num_days();
                remaining.max(0)
            });

            (
                StatusCode::OK,
                Json(BillingStatus {
                    status,
                    current_period_end: current_period_end.map(|t| t.to_rfc3339()),
                    refund_days_remaining: refund_days_remaining.filter(|&d| d > 0),
                    paddle_customer_id,
                }),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::OK,
            Json(BillingStatus {
                status: "none".into(),
                current_period_end: None,
                refund_days_remaining: None,
                paddle_customer_id: None,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(user_id = %user.id, error = %e, "billing status query failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── GET /metering/summary ─────────────────────────────────────────────────────

#[derive(Serialize)]
struct MeteringSummary {
    agent_invocations_used: i64,
    agent_invocations_limit: i64,
    spend_usd: f64,
    spend_cap_usd: f64,
    period_start: String,
}

async fn metering_summary(
    State(metering_svc): State<Arc<MeteringService>>,
    auth: AuthSession<KayaAuthBackend>,
) -> Response {
    let user = match auth.user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match metering_svc.monthly_summary(user.id).await {
        Ok(summary) => {
            let now = chrono::Utc::now();
            let period_start = chrono::NaiveDate::from_ymd_opt(
                now.year(),
                now.month(),
                1,
            )
            .unwrap()
            .to_string();

            (
                StatusCode::OK,
                Json(MeteringSummary {
                    agent_invocations_used: summary.agent_invocations,
                    agent_invocations_limit: metering_svc.included_invocations() as i64,
                    spend_usd: summary.cost_usd,
                    spend_cap_usd: metering_svc.spend_cap_usd(),
                    period_start,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(user_id = %user.id, error = %e, "metering summary query failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
