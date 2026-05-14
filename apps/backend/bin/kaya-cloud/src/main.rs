// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! kaya-cloud — hosted cloud binary (BSL 1.1).
//!
//! # Startup sequence
//!
//! 1. Connect to Postgres (NEON_DATABASE_URL env var).
//! 2. Run storage migrations (kaya-postgres-storage MIGRATOR).
//! 3. Run session-store migration (tower-sessions-sqlx-store).
//! 4. Load pricing config (PRICING_CONFIG_PATH or the bundled default).
//! 5. Build service layer: MagicLink, Billing, Metering.
//! 6. Mount auth, account, billing, and admin routes.
//! 7. Bind and serve.
//!
//! # Environment variables
//!
//! | Variable                  | Description                                              |
//! |---------------------------|----------------------------------------------------------|
//! | `NEON_DATABASE_URL`       | Postgres connection string (required)                    |
//! | `RESEND_API_KEY`          | Resend API key for email delivery (required)             |
//! | `RESEND_FROM`             | Verified sender address (required)                       |
//! | `FRONTEND_BASE_URL`       | Frontend origin for magic-link URLs (required)           |
//! | `PADDLE_API_KEY`          | Paddle API key for REST calls (required)                 |
//! | `PADDLE_WEBHOOK_SECRET`   | Paddle webhook signing secret (required)                 |
//! | `PADDLE_API_BASE`         | Paddle API base URL (default: sandbox)                   |
//! | `PADDLE_OVERAGE_PRICE_ID` | Paddle price ID for overage billing (optional)           |
//! | `ADMIN_EMAIL`             | Hardcoded admin email for founder dashboard (required)   |
//! | `PRICING_CONFIG_PATH`     | Path to pricing.yaml (default: config/pricing.yaml)      |
//! | `PORT`                    | Bind port (default: 3001)                               |

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use kaya_billing::BillingService;
use kaya_metering::pricing::PricingConfig;
use kaya_metering::service::MeteringConfig;
use kaya_metering::MeteringService;
use kaya_tenant::{MagicLinkService, PostgresStore};
use tower_http::cors::{Any, CorsLayer};
use tower_sessions::SessionManagerLayer;
use tower_sessions::cookie::SameSite;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

mod routes;
mod state;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url = require_env("NEON_DATABASE_URL")?;
    let resend_api_key = require_env("RESEND_API_KEY")?;
    let resend_from = require_env("RESEND_FROM")?;
    let frontend_base_url = require_env("FRONTEND_BASE_URL")?;
    let paddle_api_key = require_env("PADDLE_API_KEY")?;
    let paddle_webhook_secret = require_env("PADDLE_WEBHOOK_SECRET")?;
    let admin_email = require_env("ADMIN_EMAIL")?;
    let paddle_api_base = std::env::var("PADDLE_API_BASE")
        .unwrap_or_else(|_| "https://sandbox-api.paddle.com".into());
    let paddle_overage_price_id = std::env::var("PADDLE_OVERAGE_PRICE_ID").ok();
    let pricing_config_path = std::env::var("PRICING_CONFIG_PATH")
        .unwrap_or_else(|_| "config/pricing.yaml".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    let pool = sqlx::PgPool::connect(&database_url).await?;
    tracing::info!("connected to Postgres");

    kaya_postgres_storage::MIGRATOR.run(&pool).await?;
    tracing::info!("storage migrations applied");

    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;
    tracing::info!("session store ready");

    let magic_link_svc = Arc::new(MagicLinkService::new(
        pool.clone(),
        resend_api_key.clone(),
        resend_from.clone(),
        frontend_base_url,
    ));

    let billing_svc = Arc::new(BillingService::new(
        pool.clone(),
        paddle_api_key.clone(),
        paddle_api_base.clone(),
        paddle_webhook_secret,
    ));

    let pricing = PricingConfig::from_yaml_file(Path::new(&pricing_config_path))
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "pricing config not found, using empty config");
            PricingConfig { models: Default::default() }
        });

    let metering_config = MeteringConfig {
        spend_cap_usd: 6.00,
        alert_threshold: 0.80,
        included_invocations: 50,
        hourly_token_limit: 100_000,
        daily_token_limit: 500_000,
        circuit_threshold_usd: 50.00,
        paddle_api_key: paddle_api_key.clone(),
        paddle_api_base: paddle_api_base.clone(),
        paddle_overage_price_id,
        resend_api_key,
        resend_from,
        admin_email: admin_email.clone(),
    };
    let metering_svc = Arc::new(MeteringService::new(pool.clone(), pricing, metering_config));

    let state = AppState {
        pool: pool.clone(),
        magic_link_svc: magic_link_svc.clone(),
        billing_svc,
        metering_svc,
        admin_email,
    };

    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("kaya_session")
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(true)
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::days(7),
        ));

    let backend = kaya_tenant::KayaAuthBackend::new(pool.clone(), magic_link_svc);
    let auth_layer = axum_login::AuthManagerLayerBuilder::new(backend, session_layer).build();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::auth::router())
        .merge(routes::account::router())
        .merge(routes::billing::router())
        .merge(routes::admin::router())
        .layer(auth_layer)
        .layer(cors)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(port = port, "kaya-cloud listening");

    axum::serve(listener, app).await?;
    Ok(())
}

fn require_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var: {key}"))
}
