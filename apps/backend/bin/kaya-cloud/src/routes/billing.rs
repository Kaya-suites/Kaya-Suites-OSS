// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Billing routes.
//!
//! - `POST /webhooks/paddle` — receive Paddle webhook events (no auth)
//! - `POST /billing/refund`  — 30-day money-back refund (requires auth)

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use kaya_billing::{BillingError, BillingService, PaddleWebhookPayload};
use kaya_tenant::{AuthSession, KayaAuthBackend};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhooks/paddle", post(paddle_webhook))
        .route("/billing/refund", post(request_refund))
}

// ── POST /webhooks/paddle ─────────────────────────────────────────────────────

async fn paddle_webhook(
    State(billing_svc): State<Arc<BillingService>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let sig_header = match headers
        .get("Paddle-Signature")
        .and_then(|v| v.to_str().ok())
    {
        Some(h) => h.to_owned(),
        None => {
            tracing::warn!("Paddle webhook missing signature header");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    if let Err(e) = billing_svc.verify_webhook(&sig_header, &body) {
        tracing::warn!(error = %e, "Paddle webhook signature verification failed");
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let payload: PaddleWebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "failed to parse Paddle webhook payload");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    if let Err(e) = billing_svc.handle_event(&payload).await {
        tracing::error!(
            error = %e,
            event_type = %payload.event_type,
            notification_id = %payload.notification_id,
            "Paddle webhook handling failed"
        );
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    StatusCode::OK.into_response()
}

// ── POST /billing/refund ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RefundBody {
    #[allow(dead_code)]
    reason: Option<String>,
}

#[derive(Serialize)]
struct RefundResponse {
    ok: bool,
}

async fn request_refund(
    State(billing_svc): State<Arc<BillingService>>,
    auth: AuthSession<KayaAuthBackend>,
    Json(_body): Json<RefundBody>,
) -> Response {
    let user = match auth.user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match billing_svc.request_refund(user.id).await {
        Ok(()) => (StatusCode::OK, Json(RefundResponse { ok: true })).into_response(),
        Err(BillingError::SubscriptionNotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no active subscription found"})),
        )
            .into_response(),
        Err(BillingError::RefundWindowClosed) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "refund window closed — the 30-day money-back guarantee has expired"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(user_id = %user.id, error = %e, "refund request failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
