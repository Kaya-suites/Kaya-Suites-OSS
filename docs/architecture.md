# Architecture

## Overview

Kaya Suites is an AI-native knowledge base where an agent actively maintains documents as a living source of truth. It ships as two distinct distribution surfaces:

| Surface | License | Storage | Auth |
|---|---|---|---|
| OSS self-host (`kaya-oss`) | Apache 2.0 | SQLite + sqlite-vec | None (single user) |
| Hosted cloud (`kaya-cloud`) | BSL 1.1 | Postgres + pgvector | Magic-link |

## Repository layout

```
apps/
  web/              Next.js 16 frontend
  backend/          Rust backend (Cargo workspace root at apps/backend/Cargo.toml)
packages/
  api-client/       Generated TypeScript client (from OpenAPI schema)
  ui/               Shared React components
docs/
  (this directory)  Apache 2.0 documentation
  ee/               BSL 1.1 documentation — stripped from OSS mirror
scripts/
  strip-ee.sh       Removes all ee/ directories before public mirror sync
```

## Two independent build systems

The frontend and backend are kept fully independent:

- `apps/web/` — Next.js 16, managed by **pnpm**. Not added to the Cargo workspace.
- `apps/backend/` — Rust, managed by **Cargo**. Not added to `pnpm-workspace.yaml`.

The only shared surface is `packages/api-client/`, a generated TypeScript client consumed by Next.js. See [API Codegen](api-codegen.md).

## Backend crate graph

```
bin/kaya-oss   (Apache 2.0) ──► kaya-core, kaya-storage

crates/
  kaya-core    (Apache 2.0) — traits: StorageAdapter, AuthAdapter, LlmProvider
  kaya-storage (Apache 2.0) — SqliteAdapter implementation
```

The hosted cloud distribution adds further crates under `ee/` (BSL 1.1, not included in this mirror).

## Key architectural seams

Four enforced seams allow the OSS and cloud distributions to share the same core:

1. **[StorageAdapter](storage-adapter.md)** — swaps SQLite ↔ Postgres without changing business logic.
2. **[AuthAdapter](auth-adapter.md)** — swaps single-user ↔ magic-link auth.
3. **[LlmProvider](llm-provider.md)** — vendor-agnostic interface; no SDK import outside provider files.
4. **Propose-then-approve** — `ApprovalToken` has a `pub(crate)` constructor; only `UserSession::approve_edit` can produce one. Enforced by `trybuild` compile-fail tests in `crates/kaya-core/tests/`.

## Next.js frontend layout

```
apps/web/app/
  (shared)/       Apache 2.0 routes — compiled into both OSS and cloud
  (ee)/           BSL 1.1 routes    — stripped from OSS mirror (route group, no URL prefix)
apps/web/components/
  shared/         Apache 2.0 components
  ee/             BSL 1.1 components
```

`NEXT_PUBLIC_API_URL` controls the backend base URL (default: `http://localhost:3001`).
