// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! kaya-cloud — hosted cloud binary (BSL 1.1).
//!
//! # Startup sequence
//!
//! 1. Connect to Postgres (NEON_DATABASE_URL env var).
//! 2. Run storage migrations (kaya-postgres-storage MIGRATOR).
//! 3. Run session-store migration (tower-sessions-sqlx-store).
//! 4. Build the tower layer stack: CORS → trace → sessions → axum-login.
//! 5. Mount auth and account routes.
//! 6. Bind and serve.
//!
//! # Environment variables
//!
//! | Variable               | Description                                     |
//! |------------------------|-------------------------------------------------|
//! | `NEON_DATABASE_URL`    | Postgres connection string (required)           |
//! | `RESEND_API_KEY`       | Resend API key for email delivery (required)    |
//! | `RESEND_FROM`          | Verified sender address (required)              |
//! | `FRONTEND_BASE_URL`    | Frontend origin for magic-link URLs (required)  |
//! | `PORT`                 | Bind port (default: 3001)                       |

use std::sync::Arc;

use axum::Router;
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
        resend_api_key,
        resend_from,
        frontend_base_url,
    ));
    let state = AppState {
        pool: pool.clone(),
        magic_link_svc: magic_link_svc.clone(),
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
