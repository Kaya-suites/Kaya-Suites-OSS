//! Postgres + pgvector storage adapter for Kaya Suites cloud (BSL 1.1).
//!
//! Will implement `StorageAdapter` from `kaya-core` on top of `sqlx` with the
//! `postgres` feature and the `pgvector` extension for similarity search.
//!
//! # Multi-tenancy contract
//! The adapter's constructor takes a `UserContext` from `kaya-tenant`.
//! All query methods are on the scoped instance — there are no static query
//! methods. This is a structural guarantee: the compiler prevents any code
//! path that forgets to scope queries to a tenant.
//!
//! Not yet implemented — placeholder crate for license-boundary scaffolding.
