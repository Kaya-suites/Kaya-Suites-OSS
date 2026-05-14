// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Global daily spend circuit breaker (BRD §12.5).
//!
//! When aggregate daily spend across all users exceeds `threshold_usd`, new
//! agent invocations are rejected with `MeteringError::CircuitBreakerOpen`.
//!
//! State is cached in an `AtomicBool` and refreshed from the DB at most once
//! per `check_interval` (default: 60 s).  On trip the state is also written
//! to `system_flags` for persistence across restarts and to trigger an alert.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::PgPool;
use tracing::{error, info, warn};

use crate::error::MeteringError;

pub struct CircuitBreaker {
    tripped: Arc<AtomicBool>,
    threshold_usd: f64,
    last_check: Arc<Mutex<Option<Instant>>>,
    check_interval: Duration,
}

impl CircuitBreaker {
    pub fn new(threshold_usd: f64) -> Self {
        Self {
            tripped: Arc::new(AtomicBool::new(false)),
            threshold_usd,
            last_check: Arc::new(Mutex::new(None)),
            check_interval: Duration::from_secs(60),
        }
    }

    /// Check the circuit breaker.  Returns `MeteringError::CircuitBreakerOpen`
    /// if the daily aggregate spend has breached the threshold.
    ///
    /// The DB is queried at most once per `check_interval`; subsequent calls
    /// within the window use the cached `AtomicBool`.
    pub async fn check(&self, pool: &PgPool) -> Result<(), MeteringError> {
        if self.tripped.load(Ordering::Relaxed) {
            let daily = self.daily_spend(pool).await.unwrap_or(f64::MAX);
            return Err(MeteringError::CircuitBreakerOpen {
                daily_usd: daily,
                threshold_usd: self.threshold_usd,
            });
        }

        let needs_refresh = {
            let last = self.last_check.lock().expect("circuit lock poisoned");
            last.map_or(true, |t| t.elapsed() > self.check_interval)
        };

        if !needs_refresh {
            return Ok(());
        }

        let daily = self.daily_spend(pool).await?;
        *self.last_check.lock().expect("circuit lock poisoned") = Some(Instant::now());

        if daily >= self.threshold_usd {
            self.tripped.store(true, Ordering::Relaxed);
            warn!(
                daily_usd = daily,
                threshold_usd = self.threshold_usd,
                "circuit breaker TRIPPED — blocking new agent invocations"
            );
            self.persist_trip(pool, daily).await;
            return Err(MeteringError::CircuitBreakerOpen {
                daily_usd: daily,
                threshold_usd: self.threshold_usd,
            });
        }

        Ok(())
    }

    /// Reset the circuit breaker (founder-initiated, after investigating the anomaly).
    pub async fn reset(&self, pool: &PgPool) {
        self.tripped.store(false, Ordering::Relaxed);
        *self.last_check.lock().expect("circuit lock poisoned") = None;
        let _ = sqlx::query(
            "DELETE FROM system_flags WHERE key = 'circuit_breaker_tripped'",
        )
        .execute(pool)
        .await;
        info!("circuit breaker reset");
    }

    pub fn is_tripped(&self) -> bool {
        self.tripped.load(Ordering::Relaxed)
    }

    async fn daily_spend(&self, pool: &PgPool) -> Result<f64, MeteringError> {
        let spend: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(cost_usd), 0.0)::float8
             FROM usage_events
             WHERE recorded_at >= date_trunc('day', now() AT TIME ZONE 'UTC')",
        )
        .fetch_one(pool)
        .await?;
        Ok(spend)
    }

    async fn persist_trip(&self, pool: &PgPool, daily_usd: f64) {
        let value = format!("{:.6}", daily_usd);
        let res = sqlx::query(
            "INSERT INTO system_flags (key, value) VALUES ('circuit_breaker_tripped', $1)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
        )
        .bind(&value)
        .execute(pool)
        .await;
        if let Err(e) = res {
            error!(error = %e, "failed to persist circuit breaker state");
        }
    }
}
