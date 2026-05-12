//! SQLite storage adapter for Kaya Suites OSS (Apache 2.0).
//!
//! # BRD note
//! The `StorageAdapter` trait was moved to `kaya-core` (rather than living here
//! as the BRD originally specified) to avoid a circular dependency with
//! `commit_edit`. TODO: flag in BRD §8 revision.
//!
//! # Design
//! Files on disk (`.md` with YAML frontmatter) are the source of truth.
//! SQLite is a fast index for listing and search. `get_document` always reads
//! from disk; the index is only used to map UUIDs to file paths.

pub mod document;
pub mod sqlite;

// Re-export the trait so callers can depend on only this crate.
pub use kaya_core::StorageAdapter;
pub use sqlite::SqliteAdapter;
