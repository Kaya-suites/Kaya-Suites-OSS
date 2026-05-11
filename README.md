# Kaya Suites

An AI-native knowledge base where an agent actively maintains documents as a living source of truth.

## Distribution surfaces

| Surface | License | Storage | Auth |
|---------|---------|---------|------|
| OSS self-host | Apache 2.0 | SQLite + sqlite-vec | None (single user) |
| Hosted cloud | BSL 1.1 | Postgres + pgvector | Magic-link |

## Repository layout

```
apps/
  web/          Next.js 16 frontend
  backend/      Rust backend (Cargo workspace)
packages/
  api-client/   Generated TypeScript client (from OpenAPI schema)
  ui/           Shared React components
scripts/
  strip-ee.sh   Strips BSL code before public mirror sync
```

## License boundary

Everything inside a directory named `ee/` is **BSL 1.1** and is stripped from the public mirror before sync. Everything outside `ee/` is **Apache 2.0**.

## Getting started

### Frontend
```bash
pnpm install          # from repo root
pnpm dev
```

### Backend
```bash
cd apps/backend
cargo build --workspace
cargo run --bin kaya-oss
```

### Generate API client (after backend schema changes)
```bash
# Start the backend, then:
cargo run --bin kaya-oss -- --schema > packages/api-client/openapi.json
pnpm generate
```
