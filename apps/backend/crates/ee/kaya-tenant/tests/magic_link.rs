// Integration tests for MagicLinkService.
//
// Requires a Postgres instance (no pgvector needed — magic_links has no vectors).
// Set DATABASE_URL before running:
//
//   export DATABASE_URL=postgres://user:pass@host/db
//   cargo test -p kaya-tenant
//
// sqlx::test creates an isolated database per test and drops it when done.

use kaya_tenant::{MagicLinkError, MagicLinkService};
use sqlx::PgPool;
use uuid::Uuid;

// ── Schema setup ──────────────────────────────────────────────────────────────

/// Create the minimal schema that MagicLinkService relies on.
///
/// In production this is handled by kaya-postgres-storage's MIGRATOR. Tests
/// create it inline to avoid a circular crate dependency.
async fn setup_schema(pool: &PgPool) {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS users (
            id         UUID        NOT NULL DEFAULT gen_random_uuid(),
            email      TEXT        NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            PRIMARY KEY (id),
            UNIQUE (email)
        )"#,
    )
    .execute(pool)
    .await
    .expect("create users table");

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS magic_links (
            id         UUID        NOT NULL DEFAULT gen_random_uuid(),
            email      TEXT        NOT NULL,
            token_hash TEXT        NOT NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            used_at    TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            PRIMARY KEY (id),
            UNIQUE (token_hash)
        )"#,
    )
    .execute(pool)
    .await
    .expect("create magic_links table");
}

fn make_svc(pool: PgPool) -> MagicLinkService {
    MagicLinkService::new(
        pool,
        "test_api_key_not_used",
        "noreply@test.example",
        "http://localhost:3000",
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// FR-28: a valid token can be created and verified exactly once.
#[sqlx::test]
async fn magic_link_round_trip(pool: PgPool) {
    setup_schema(&pool).await;
    let svc = make_svc(pool);

    let email = format!("{}@example.com", Uuid::new_v4());
    let token = svc
        .create_and_store_token(&email)
        .await
        .expect("create token");

    let (user_id, returned_email) = svc.verify(&token).await.expect("first verify must succeed");
    assert_eq!(returned_email, email);
    assert!(!user_id.is_nil());
}

/// A token can only be used once; the second attempt returns AlreadyUsed.
#[sqlx::test]
async fn magic_link_replay_rejected(pool: PgPool) {
    setup_schema(&pool).await;
    let svc = make_svc(pool);

    let email = format!("{}@example.com", Uuid::new_v4());
    let token = svc.create_and_store_token(&email).await.unwrap();

    svc.verify(&token).await.expect("first use must succeed");

    let err = svc.verify(&token).await.expect_err("second use must fail");
    assert!(
        matches!(err, MagicLinkError::AlreadyUsed | MagicLinkError::Invalid),
        "expected AlreadyUsed or Invalid, got: {err:?}"
    );
}

/// A token whose expiry is in the past is rejected with Expired.
#[sqlx::test]
async fn expired_token_rejected(pool: PgPool) {
    setup_schema(&pool).await;
    let svc = make_svc(pool.clone());

    let email = format!("{}@example.com", Uuid::new_v4());
    let token = svc.create_and_store_token(&email).await.unwrap();

    // Back-date the expiry so the token is already expired.
    sqlx::query("UPDATE magic_links SET expires_at = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .unwrap();

    let err = svc.verify(&token).await.expect_err("expired token must fail");
    assert!(
        matches!(err, MagicLinkError::Expired),
        "expected Expired, got: {err:?}"
    );
}

/// An unknown / garbage token returns Invalid.
#[sqlx::test]
async fn unknown_token_rejected(pool: PgPool) {
    setup_schema(&pool).await;
    let svc = make_svc(pool);

    let err = svc
        .verify("0000000000000000000000000000000000000000000000000000000000000000")
        .await
        .expect_err("unknown token must fail");

    assert!(
        matches!(err, MagicLinkError::Invalid),
        "expected Invalid, got: {err:?}"
    );
}

/// Requesting a link twice for the same email upserts the user (no duplicate rows).
#[sqlx::test]
async fn idempotent_user_upsert(pool: PgPool) {
    setup_schema(&pool).await;
    let svc = make_svc(pool.clone());

    let email = format!("{}@example.com", Uuid::new_v4());
    svc.create_and_store_token(&email).await.unwrap();
    svc.create_and_store_token(&email).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count, 1, "user row must be idempotent");
}
