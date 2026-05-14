# PostgresAdapter

**Crate:** `crates/ee/kaya-postgres-storage/`  
**License:** BSL 1.1  
**Status:** Not yet implemented (trait only in `kaya-core`)

## Overview

`PostgresAdapter` is the cloud implementation of `StorageAdapter`. It replaces the in-process SQLite backend with Postgres + pgvector, enabling:

- Multi-tenant data isolation (one schema or row-level-security policy per org)
- Server-side approximate nearest-neighbour search via `pgvector`
- Managed hosting on Neon DB

## Multi-tenancy seam

The `PostgresAdapter` constructor takes a `UserContext` (resolved from the session cookie by `CloudAuthAdapter`). All query methods are scoped to that context — there are no static query methods. This prevents cross-tenant data access at the type level.

```rust
// Cloud binary startup:
let adapter = PostgresAdapter::new(pool.clone(), user_ctx);
// All subsequent calls are automatically scoped to user_ctx.org_id
adapter.list_documents().await?;
```

## Migrations

Migrations live in `crates/ee/kaya-postgres-storage/migrations/` and are numbered sequentially:

| Migration | Content |
|---|---|
| `001_schema.sql` | Core `documents`, `chunks`, `embeddings` tables |
| `002_fts.sql` | Full-text search configuration |
| `003_tenancy.sql` | Organisation and row-level-security setup |
| `004_metering.sql` | Token metering and spend tracking tables |

Run migrations via the `kaya-cloud` binary at startup:

```bash
kaya-cloud migrate
```

## `search_embeddings` implementation

In `PostgresAdapter`, `search_embeddings` delegates to pgvector's `<=>` cosine distance operator, executing an ANN index scan server-side. This is in contrast to the `SqliteAdapter`, which loads all embeddings into Rust and computes cosine similarity in process.

```sql
SELECT document_id, paragraph_id, content
FROM embeddings
ORDER BY vector <=> $1
LIMIT $2;
```

## Configuration

`PostgresAdapter` reads the database URL from the `DATABASE_URL` environment variable at startup. Neon DB connection strings are supported natively via the `tokio-postgres` driver.

| Variable | Description |
|---|---|
| `DATABASE_URL` | Postgres connection string (e.g. `postgres://…@…/kaya`) |
| `PG_TEST_DATABASE_URL` | Test database (integration tests skip without this) |
