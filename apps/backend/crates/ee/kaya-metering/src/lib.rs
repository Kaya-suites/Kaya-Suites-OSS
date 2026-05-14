// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Token usage metering, spend caps, and rate limits for Kaya Suites cloud.
//!
//! # Architecture
//!
//! - [`MeteringService`] is the public façade.  Hold behind an `Arc`.
//! - [`MeteringConfig`] controls all tunable parameters (caps, limits, keys).
//! - [`PricingConfig`] maps model names to USD cost per million tokens.
//!
//! # Agent loop integration
//!
//! ```ignore
//! // Before each agent invocation:
//! metering_svc.pre_invocation_check(user_id).await?;
//!
//! // After each LLM call:
//! metering_svc.record_usage(user_id, &token_usage).await?;
//! ```
//!
//! # See also
//!
//! `CONFIG.md` at the repo root documents the resolved values for D-12,
//! D-13, and D-14 and the cost model that produced them.

pub mod aggregation;
pub mod caps;
pub mod circuit;
pub mod error;
pub mod events;
pub mod overage;
pub mod pricing;
pub mod rate_limit;
pub mod service;

pub use aggregation::{AdminStats, UsageSummary, UserStats};
pub use error::MeteringError;
pub use pricing::PricingConfig;
pub use service::{MeteringConfig, MeteringService};
