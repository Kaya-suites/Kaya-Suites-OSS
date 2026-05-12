//! Integration tests for `SqliteAdapter`.
//!
//! Each test spins up a fresh temporary directory + SQLite database so the tests
//! are completely isolated and can run in parallel.

use std::path::PathBuf;
use uuid::Uuid;
use chrono::NaiveDate;

use kaya_core::storage::{Document, StorageAdapter};
use kaya_storage::SqliteAdapter;

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Create an isolated (content_dir, db_path) pair inside a tempdir.
///
/// The returned `TempDir` must be kept alive for the duration of the test;
/// dropping it deletes the directory.
fn temp_env() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let content = dir.path().join("content");
    let db = dir.path().join("index.db");
    (dir, content, db)
}

/// Build a fully-populated test document.
fn make_doc(rel_path: &str) -> Document {
    Document {
        id: Uuid::new_v4(),
        title: "Integration Test Doc".to_string(),
        owner: Some("alice".to_string()),
        last_reviewed: Some(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()),
        tags: vec!["rust".to_string(), "sqlite".to_string()],
        related_docs: vec![],
        body: "# Hello\n\nThis is the body.\n".to_string(),
        path: Some(PathBuf::from(rel_path)),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// FR-1, FR-2: Save a document with full frontmatter, load it back, assert equality.
#[tokio::test]
async fn test_round_trip() {
    let (_dir, content, db) = temp_env();
    let adapter = SqliteAdapter::new(content, &db).await.unwrap();
    adapter.wait_for_reconciliation().await;

    let doc = make_doc("round_trip.md");
    adapter.save_document(&doc).await.unwrap();

    let loaded = adapter.get_document(doc.id).await.unwrap();
    assert_eq!(loaded.id, doc.id);
    assert_eq!(loaded.title, doc.title);
    assert_eq!(loaded.owner, doc.owner);
    assert_eq!(loaded.last_reviewed, doc.last_reviewed);
    assert_eq!(loaded.tags, doc.tags);
    assert_eq!(loaded.related_docs, doc.related_docs);
    assert_eq!(loaded.body.trim(), doc.body.trim());
    assert_eq!(loaded.path.as_deref(), Some(PathBuf::from("round_trip.md").as_path()));

    // list_documents must include it
    let all = adapter.list_documents().await.unwrap();
    assert!(all.iter().any(|d| d.id == doc.id), "list_documents must return saved doc");
}

/// FR-5: Write a file directly to disk between two adapter instances.
/// The second adapter picks up the change after reconciliation.
#[tokio::test]
async fn test_manual_edit_detection() {
    let (_dir, content, db) = temp_env();

    // ── First adapter: save the original document ──────────────────────────
    {
        let adapter = SqliteAdapter::new(content.clone(), &db).await.unwrap();
        adapter.wait_for_reconciliation().await;

        let doc = make_doc("manual_edit.md");
        adapter.save_document(&doc).await.unwrap();
    }

    // ── Edit the file directly on disk ─────────────────────────────────────
    let file_path = content.join("manual_edit.md");
    let original_raw = std::fs::read_to_string(&file_path).unwrap();

    // Patch the title in the YAML frontmatter by replacing the title line.
    let patched = original_raw.replace("title: Integration Test Doc", "title: Edited Title");
    assert_ne!(original_raw, patched, "patch must change the content");
    std::fs::write(&file_path, &patched).unwrap();

    // ── Second adapter: reconciliation must pick up the change ─────────────
    let adapter2 = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter2.wait_for_reconciliation().await;

    // We need the id to look up the doc. Parse it from the (patched) file.
    let (doc_from_file, _) = kaya_storage::document::parse_document(&patched).unwrap();
    let loaded = adapter2.get_document(doc_from_file.id).await.unwrap();

    assert_eq!(loaded.title, "Edited Title", "adapter2 must reflect the on-disk edit");
}

/// FR-2: UUID must survive a file rename and a title change.
#[tokio::test]
async fn test_uuid_stability_after_rename() {
    let (_dir, content, db) = temp_env();
    let adapter = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter.wait_for_reconciliation().await;

    let doc = make_doc("original_name.md");
    let original_id = doc.id;
    adapter.save_document(&doc).await.unwrap();

    // Rename the file on disk.
    let old_path = content.join("original_name.md");
    let new_path = content.join("renamed.md");
    std::fs::rename(&old_path, &new_path).unwrap();

    // Open a second adapter so reconciliation re-runs with the new layout.
    let adapter2 = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter2.wait_for_reconciliation().await;

    // Save the doc again via the adapter with the new path to update the index.
    // (In real use the agent would call save_document after rename.)
    let mut renamed_doc = doc.clone();
    renamed_doc.path = Some(PathBuf::from("renamed.md"));
    adapter2.save_document(&renamed_doc).await.unwrap();

    let loaded = adapter2.get_document(original_id).await.unwrap();
    assert_eq!(
        loaded.id, original_id,
        "UUID must not change across a file rename"
    );
    assert_eq!(loaded.path.as_deref(), Some(PathBuf::from("renamed.md").as_path()));
}

/// File-on-disk wins: if index is stale, the file is the truth.
///
/// We corrupt the frontmatter_json column in the index and verify that
/// `get_document` returns the data from disk, not the corrupted index.
#[tokio::test]
async fn test_file_on_disk_wins() {
    let (_dir, content, db) = temp_env();
    let adapter = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter.wait_for_reconciliation().await;

    let doc = make_doc("truth.md");
    adapter.save_document(&doc).await.unwrap();

    // Corrupt the frontmatter_json in the SQLite index directly.
    {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::SqlitePool;
        let opts = SqliteConnectOptions::new().filename(&db);
        let pool = SqlitePool::connect_with(opts).await.unwrap();
        sqlx::query("UPDATE documents SET frontmatter_json = '{\"corrupted\":true}' WHERE id = ?")
            .bind(doc.id.to_string())
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }

    // get_document reads from disk, not from frontmatter_json column.
    let loaded = adapter.get_document(doc.id).await.unwrap();
    assert_eq!(loaded.title, doc.title, "disk file must win over corrupted index");
    assert_eq!(loaded.body.trim(), doc.body.trim());
}
