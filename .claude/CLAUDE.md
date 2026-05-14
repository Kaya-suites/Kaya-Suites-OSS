# Kaya Suites — Architectural Commitments for Claude

This file is the canonical reference for all future Claude sessions. Read it in full before writing any code.

## Two independent build systems

- `apps/web/` — Next.js 16, managed by pnpm. Do NOT add it to the Cargo workspace.
- `apps/backend/` — Rust, managed by Cargo. Do NOT add it to `pnpm-workspace.yaml`.
- The only shared surface is `packages/api-client/`, a generated TypeScript client consumed by Next.js.

## Rust edition

All crates use `edition = "2024"`.

## Cargo workspace root

`apps/backend/Cargo.toml` is the Cargo workspace root. There is **no** `Cargo.toml` at the repo root.

## License boundary

- Anything inside a directory named `ee/` → **BSL 1.1**
- Everything else → **Apache 2.0**
- `bin/kaya-oss/Cargo.toml` lists **only** Apache crates as dependencies.
- `scripts/strip-ee.sh` strips all `ee/` directories before syncing to the public mirror.

## Non-negotiable architectural seams

### 1. StorageAdapter (`crates/kaya-core/src/storage.rs`)

Object-safe async trait (via `async-trait` crate). Methods:
`get_document`, `save_document`, `delete_document`, `list_documents`, `search_embeddings`, `save_embeddings`.

OSS implementation:
- `SqliteAdapter` — `crates/kaya-storage/`, Apache 2.0

> NOTE: The brief placed the trait in `kaya-storage`, but it lives in `kaya-core` to avoid a circular dependency with `commit_edit`. Document this when revising the BRD.

### 2. AuthAdapter (`crates/kaya-core/src/auth.rs`)

Methods: `current_user`, `require_auth`.
- `LocalAuthAdapter` (Apache) — returns a fixed single user, no network call

### 3. LlmProvider (`crates/kaya-core/src/model_router.rs`)

Methods: `complete`, `stream`, `embed`, `tool_call`.
No code outside the provider implementation files imports a vendor SDK.

### 4. Propose-then-approve is structural

`ApprovalToken` has private fields and a `pub(crate)` constructor. External code cannot fabricate one. Only `UserSession::approve_edit` (a public method) can produce a token. Enforced by `trybuild` compile-fail tests in `crates/kaya-core/tests/`.

## OpenAPI codegen pipeline

1. Rust binary emits schema: `cargo run --bin kaya-oss -- --schema > packages/api-client/openapi.json`
2. TypeScript client is generated: `pnpm generate` (runs `@hey-api/openapi-ts`)
3. Generated output lives in `packages/api-client/src/` and is committed.

## Next.js conventions

- Target: **Next.js 16.x** (currently 16.2.6). Flag any tooling that defaults to 15.
- App router only. No pages router.
- Route layout: `app/(shared)/` for Apache routes. Route groups do **not** add a URL prefix.
- Component layout: `components/shared/`.
- Backend URL: `NEXT_PUBLIC_API_URL`, default `http://localhost:3001`.

## What has NOT been implemented yet

- No AuthAdapter implementation (trait only)

@.claude/ee/enterprise.md
