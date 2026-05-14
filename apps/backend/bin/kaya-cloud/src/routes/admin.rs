// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Founder admin routes — auth-gated to `ADMIN_EMAIL`.
//!
//! - `GET  /admin/stats`           — aggregate spend, top users, circuit state
//! - `POST /admin/circuit-breaker/reset` — reset a tripped circuit breaker

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use kaya_metering::MeteringService;
use kaya_tenant::{AuthSession, KayaAuthBackend};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/stats", get(admin_stats))
        .route("/admin/circuit-breaker/reset", post(reset_circuit_breaker))
}

async fn admin_stats(
    State(metering): State<Arc<MeteringService>>,
    State(state): State<AppState>,
    auth: AuthSession<KayaAuthBackend>,
) -> Response {
    let user = match auth.user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if user.email != state.admin_email {
        return StatusCode::FORBIDDEN.into_response();
    }

    match metering.admin_stats().await {
        Ok(stats) => (StatusCode::OK, Json(stats)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "admin_stats failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn reset_circuit_breaker(
    State(metering): State<Arc<MeteringService>>,
    State(state): State<AppState>,
    auth: AuthSession<KayaAuthBackend>,
) -> Response {
    let user = match auth.user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if user.email != state.admin_email {
        return StatusCode::FORBIDDEN.into_response();
    }

    metering.reset_circuit_breaker().await;
    (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
}
