// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1

use std::sync::Arc;

use axum::extract::FromRef;
use kaya_billing::BillingService;
use kaya_metering::MeteringService;
use kaya_tenant::MagicLinkService;
use sqlx::PgPool;

/// Shared application state for the cloud binary.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub magic_link_svc: Arc<MagicLinkService>,
    pub billing_svc: Arc<BillingService>,
    pub metering_svc: Arc<MeteringService>,
    /// Hardcoded admin email from `ADMIN_EMAIL` env var.
    pub admin_email: String,
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

impl FromRef<AppState> for Arc<BillingService> {
    fn from_ref(s: &AppState) -> Self {
        s.billing_svc.clone()
    }
}

impl FromRef<AppState> for Arc<MeteringService> {
    fn from_ref(s: &AppState) -> Self {
        s.metering_svc.clone()
    }
}
