// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Magic-link auth routes.
//!
//! - `POST /auth/request-link` — generate & email a magic link
//! - `GET  /auth/verify`       — redeem a token, create session, redirect
//! - `GET  /auth/me`           — return current user (for session-check proxy)
//! - `POST /auth/logout`       — destroy session

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use kaya_tenant::{AuthSession, KayaAuthBackend, MagicLinkError, MagicLinkService};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/request-link", post(request_link))
        .route("/auth/verify", get(verify))
        .route("/auth/me", get(me))
        .route("/auth/logout", post(logout))
}

// ── POST /auth/request-link ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct RequestLinkBody {
    email: String,
}

#[derive(Serialize)]
struct RequestLinkResponse {
    ok: bool,
}

async fn request_link(
    State(svc): State<Arc<MagicLinkService>>,
    Json(body): Json<RequestLinkBody>,
) -> Response {
    match svc.request_link(&body.email).await {
        Ok(()) => Json(RequestLinkResponse { ok: true }).into_response(),
        Err(MagicLinkError::EmailDelivery(msg)) => {
            tracing::error!(email = %body.email, error = %msg, "email delivery failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "request_link failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── GET /auth/verify?token=... ────────────────────────────────────────────────

#[derive(Deserialize)]
struct VerifyParams {
    token: String,
}

async fn verify(
    State(svc): State<Arc<MagicLinkService>>,
    mut auth: AuthSession<KayaAuthBackend>,
    Query(params): Query<VerifyParams>,
) -> Response {
    match svc.verify(&params.token).await {
        Ok((user_id, email)) => {
            let user = kaya_tenant::AuthUser { id: user_id, email };
            if let Err(e) = auth.login(&user).await {
                tracing::error!(error = %e, "session login failed");
                return (StatusCode::INTERNAL_SERVER_ERROR, "session error").into_response();
            }
            Redirect::to("/").into_response()
        }
        Err(MagicLinkError::Expired) => error_page(
            StatusCode::GONE,
            "Link expired",
            "This sign-in link has expired. Please request a new one.",
        ),
        Err(MagicLinkError::AlreadyUsed) => error_page(
            StatusCode::GONE,
            "Link already used",
            "This sign-in link has already been used. Please request a new one.",
        ),
        Err(_) => error_page(
            StatusCode::BAD_REQUEST,
            "Invalid link",
            "This sign-in link is not valid. Please request a new one.",
        ),
    }
}

// ── GET /auth/me ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MeResponse {
    user_id: String,
    email: String,
}

async fn me(auth: AuthSession<KayaAuthBackend>) -> Response {
    match auth.user {
        Some(user) => Json(MeResponse {
            user_id: user.id.to_string(),
            email: user.email,
        })
        .into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

// ── POST /auth/logout ─────────────────────────────────────────────────────────

async fn logout(mut auth: AuthSession<KayaAuthBackend>) -> Response {
    if let Err(e) = auth.logout().await {
        tracing::error!(error = %e, "logout failed");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn error_page(status: StatusCode, title: &str, message: &str) -> Response {
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>{title}</title>
<style>body{{font-family:-apple-system,sans-serif;max-width:480px;margin:80px auto;padding:0 20px;color:#111}}
h1{{font-size:20px}}p{{color:#555}}a{{color:#111}}</style></head>
<body>
  <h1>{title}</h1>
  <p>{message}</p>
  <p><a href="/ee/auth/signin">Request a new sign-in link &rarr;</a></p>
</body></html>"#
    );
    (status, [("content-type", "text/html; charset=utf-8")], html).into_response()
}
