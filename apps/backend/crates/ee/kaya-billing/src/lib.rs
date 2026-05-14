// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Paddle billing integration for Kaya Suites cloud.
//!
//! # Overview
//!
//! - [`BillingService`] — webhook event handling and 30-day refund logic.
//! - [`verify_webhook_signature`] — standalone HMAC-SHA256 verifier used in
//!   the Axum route handler before calling `BillingService::handle_event`.
//! - [`SubscriptionStatus`] — Kaya's lifecycle states mapped from Paddle.
//! - [`BillingError`] — unified error type for all billing operations.

pub mod types;
pub mod webhook;

pub use types::{BillingError, SubscriptionStatus};
pub use webhook::{BillingService, PaddleWebhookPayload, verify_webhook_signature};
