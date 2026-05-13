// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1

use std::sync::Arc;

use axum::extract::FromRef;
use kaya_tenant::MagicLinkService;
use sqlx::PgPool;

/// Shared application state for the cloud binary.
///
/// `FromRef` impls allow axum handlers to extract `PgPool` or
/// `Arc<MagicLinkService>` directly from `State<AppState>` without
/// exposing the full state struct to every handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub magic_link_svc: Arc<MagicLinkService>,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(s: &AppState) -> Self {
        s.pool.clone()
    }
}

impl FromRef<AppState> for Arc<MagicLinkService> {
    fn from_ref(s: &AppState) -> Self {
        s.magic_link_svc.clone()
    }
}
