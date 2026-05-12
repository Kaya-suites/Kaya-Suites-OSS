//! SQLite storage adapter for Kaya Suites OSS (Apache 2.0).
//!
//! # BRD note
//! The brief specifies that `StorageAdapter` is defined in this crate.
//! It was moved to `kaya-core` to avoid a circular dependency with
//! `commit_edit`. TODO: flag in BRD §8 revision.
//!
//! # Planned implementation
//! `SqliteAdapter` will be added here once the schema is designed.
//! It will use `sqlx` with the `sqlite` feature and `sqlite-vec` for
//! vector search.

// Re-export the trait so callers can depend on only this crate if they prefer.
pub use kaya_core::StorageAdapter;
