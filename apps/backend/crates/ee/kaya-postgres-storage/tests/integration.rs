// Integration tests for PostgresAdapter.
//
// These tests require a Postgres instance with pgvector installed.
// Set DATABASE_URL before running:
//
//   export DATABASE_URL=postgres://user:pass@host/db
//   cargo test -p kaya-postgres-storage
//
// sqlx::test creates a temporary database per test, applies all migrations via
// MIGRATOR, and drops the database when the test completes.

use kaya_core::storage::{Chunk, Document, Embedding, StorageAdapter, StorageError};
use kaya_postgres_storage::{PostgresAdapter, MIGRATOR};
use kaya_tenant::UserContext;
use sqlx::PgPool;
use uuid::Uuid;

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Insert a row into `users` so FK constraints on other tables are satisfied.
async fn create_test_user(pool: &PgPool, user_id: Uuid) {
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user_id)
        .bind(format!("{user_id}@test.example"))
        .execute(pool)
        .await
        .expect("insert test user");
}

fn make_doc(user_id: Uuid) -> Document {
    Document {
        id: Uuid::new_v4(),
        title: "Test document".to_string(),
        owner: Some("alice".to_string()),
        last_reviewed: None,
        tags: vec!["rust".to_string(), "test".to_string()],
        related_docs: vec![],
        body: "First paragraph.\n\nSecond paragraph.".to_string(),
        path: None,
    }
}

fn make_user_ctx(user_id: Uuid) -> UserContext {
    UserContext { tenant_id: Uuid::new_v4(), user_id }
}

// ── Document isolation ────────────────────────────────────────────────────────

/// FR-4 / NFR §6.3: User A writes a document; User B's adapter cannot see it.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn user_b_cannot_read_user_a_document(pool: PgPool) {
    let uid_a = Uuid::new_v4();
    let uid_b = Uuid::new_v4();
    create_test_user(&pool, uid_a).await;
    create_test_user(&pool, uid_b).await;

    let adapter_a = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_a));
    let adapter_b = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_b));

    let doc = make_doc(uid_a);
    adapter_a.save_document(&doc).await.expect("save by user A");

    // User B must get NotFound, not the actual document.
    let result = adapter_b.get_document(doc.id).await;
    assert!(
        matches!(result, Err(StorageError::NotFound(_))),
        "user B must not read user A's document, got: {result:?}"
    );
}

/// User B's list_documents must not contain any of User A's documents.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn list_documents_is_scoped_to_user(pool: PgPool) {
    let uid_a = Uuid::new_v4();
    let uid_b = Uuid::new_v4();
    create_test_user(&pool, uid_a).await;
    create_test_user(&pool, uid_b).await;

    let adapter_a = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_a));
    let adapter_b = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_b));

    let doc_a1 = make_doc(uid_a);
    let doc_a2 = make_doc(uid_a);
    adapter_a.save_document(&doc_a1).await.unwrap();
    adapter_a.save_document(&doc_a2).await.unwrap();

    let list_b = adapter_b.list_documents().await.unwrap();
    assert!(list_b.is_empty(), "user B must see no documents");

    let list_a = adapter_a.list_documents().await.unwrap();
    assert_eq!(list_a.len(), 2, "user A must see both their documents");
}

/// delete_document soft-deletes and makes the document invisible to list/get.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn delete_document_hides_from_owner(pool: PgPool) {
    let uid = Uuid::new_v4();
    create_test_user(&pool, uid).await;
    let adapter = PostgresAdapter::new(pool, make_user_ctx(uid));

    let doc = make_doc(uid);
    adapter.save_document(&doc).await.unwrap();
    adapter.delete_document(doc.id).await.unwrap();

    assert!(
        matches!(adapter.get_document(doc.id).await, Err(StorageError::NotFound(_))),
        "deleted document must not be retrievable"
    );
    assert!(adapter.list_documents().await.unwrap().is_empty());
}

// ── Chunk and FTS isolation ───────────────────────────────────────────────────

/// save_chunk / search_text results are isolated per user.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn fts_search_is_scoped_to_user(pool: PgPool) {
    let uid_a = Uuid::new_v4();
    let uid_b = Uuid::new_v4();
    create_test_user(&pool, uid_a).await;
    create_test_user(&pool, uid_b).await;

    let adapter_a = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_a));
    let adapter_b = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_b));

    // User A writes a document with a distinctive phrase.
    let doc = make_doc(uid_a);
    adapter_a.save_document(&doc).await.unwrap();

    let chunk = Chunk {
        document_id: doc.id,
        paragraph_id: "para0".to_string(),
        content: "xyzzy_unique_keyword for testing".to_string(),
        ordinal: 0,
    };
    adapter_a.save_chunk(&chunk).await.unwrap();

    // User B's FTS search must return nothing.
    let hits_b = adapter_b
        .search_text("xyzzy_unique_keyword", 10)
        .await
        .unwrap();
    assert!(hits_b.is_empty(), "user B must not see user A's chunks");

    // User A's FTS search finds the chunk.
    let hits_a = adapter_a
        .search_text("xyzzy_unique_keyword", 10)
        .await
        .unwrap();
    assert_eq!(hits_a.len(), 1);
    assert_eq!(hits_a[0].paragraph_id, "para0");
}

// ── Embedding isolation ───────────────────────────────────────────────────────

/// User A's embeddings must not appear in User B's vector search results.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn vector_search_is_scoped_to_user(pool: PgPool) {
    let uid_a = Uuid::new_v4();
    let uid_b = Uuid::new_v4();
    create_test_user(&pool, uid_a).await;
    create_test_user(&pool, uid_b).await;

    let adapter_a = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_a));
    let adapter_b = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_b));

    let doc = make_doc(uid_a);
    adapter_a.save_document(&doc).await.unwrap();

    let chunk = Chunk {
        document_id: doc.id,
        paragraph_id: "para0".to_string(),
        content: "semantic search test paragraph".to_string(),
        ordinal: 0,
    };
    adapter_a.save_chunk(&chunk).await.unwrap();

    // Store a 1536-dim unit vector (all-ones normalised) for simplicity.
    let dim = 1536_usize;
    let norm = (dim as f32).sqrt();
    let unit_vec: Vec<f32> = vec![1.0 / norm; dim];

    let emb = Embedding {
        document_id: doc.id,
        paragraph_id: "para0".to_string(),
        vector: unit_vec.clone(),
    };
    adapter_a.save_embeddings(&emb).await.unwrap();

    // User B searches with the same query vector — must get no results.
    let hits_b = adapter_b.search_embeddings(&unit_vec, 5).await.unwrap();
    assert!(
        hits_b.is_empty(),
        "user B must not see user A's embeddings"
    );

    // User A's search must return the embedding.
    let hits_a = adapter_a.search_embeddings(&unit_vec, 5).await.unwrap();
    assert_eq!(hits_a.len(), 1, "user A must find their own embedding");
    assert_eq!(hits_a[0].paragraph_id, "para0");
}

// ── Chunk hash round-trip ─────────────────────────────────────────────────────

/// get_chunk_hashes returns only this user's hashes for the given document.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn chunk_hashes_are_scoped(pool: PgPool) {
    let uid_a = Uuid::new_v4();
    let uid_b = Uuid::new_v4();
    create_test_user(&pool, uid_a).await;
    create_test_user(&pool, uid_b).await;

    let doc_a = make_doc(uid_a);
    let doc_b = make_doc(uid_b);

    let adapter_a = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_a));
    let adapter_b = PostgresAdapter::new(pool.clone(), make_user_ctx(uid_b));

    adapter_a.save_document(&doc_a).await.unwrap();
    adapter_b.save_document(&doc_b).await.unwrap();

    let chunk_a = Chunk {
        document_id: doc_a.id,
        paragraph_id: "p0".to_string(),
        content: "user A content".to_string(),
        ordinal: 0,
    };
    let chunk_b = Chunk {
        document_id: doc_b.id,
        paragraph_id: "p0".to_string(),
        content: "user B content".to_string(),
        ordinal: 0,
    };
    adapter_a.save_chunk(&chunk_a).await.unwrap();
    adapter_b.save_chunk(&chunk_b).await.unwrap();

    // User A querying their document gets one hash; user B's doc returns empty.
    let hashes_a = adapter_a.get_chunk_hashes(doc_a.id).await.unwrap();
    assert_eq!(hashes_a.len(), 1);

    let hashes_b_for_a_doc = adapter_b.get_chunk_hashes(doc_a.id).await.unwrap();
    assert!(
        hashes_b_for_a_doc.is_empty(),
        "user B must not see user A's chunk hashes"
    );
}

// ── Migration idempotency ─────────────────────────────────────────────────────

/// Running the migrator twice against the same database must succeed.
/// sqlx::test already applies migrations once; we apply again manually.
#[ignore = "requires DATABASE_URL pointing to a Postgres instance with pgvector"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn migration_is_idempotent(pool: PgPool) {
    // Running again must not panic or return an error.
    MIGRATOR
        .run(&pool)
        .await
        .expect("second migration run must be idempotent");
}
