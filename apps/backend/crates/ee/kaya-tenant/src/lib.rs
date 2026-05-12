//! Multi-tenant context and isolation for Kaya Suites cloud (BSL 1.1).
//!
//! Provides `UserContext` (the per-request tenant identifier) consumed by
//! `PostgresAdapter`'s constructor to scope all queries to a single tenant.
//!
//! Not yet implemented — placeholder crate for license-boundary scaffolding.

use uuid::Uuid;

/// Per-request tenant context passed into storage adapters.
///
/// The Postgres adapter constructor takes a `UserContext` and all query methods
/// are on the scoped instance — there are no static query methods.
/// This structural constraint prevents accidental cross-tenant data leakage.
#[derive(Debug, Clone)]
pub struct UserContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
}
