# API Codegen Pipeline

The TypeScript client in `packages/api-client/` is generated from the OpenAPI schema emitted by the Rust backend. Generated output is committed to the repository so consumers do not need to run the pipeline themselves.

## Pipeline steps

```
┌─────────────────────────────────────────────────────┐
│  1. Run the backend binary with --schema flag        │
│     cargo run --bin kaya-oss -- --schema             │
│     > packages/api-client/openapi.json               │
└──────────────────────────┬──────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────┐
│  2. Generate the TypeScript client                   │
│     pnpm generate  (from repo root)                  │
│     Tool: @hey-api/openapi-ts                        │
│     Output: packages/api-client/src/                 │
└─────────────────────────────────────────────────────┘
```

## When to re-run

Re-run the pipeline whenever you add, remove, or change a public API endpoint in the backend:

```bash
# From repo root — backend must be running or built first
cargo run --bin kaya-oss -- --schema > packages/api-client/openapi.json
pnpm generate
```

Commit both `openapi.json` and the updated `packages/api-client/src/` together in the same PR.

## Consuming the client in Next.js

```ts
import { DocumentsService } from '@kaya/api-client';

const docs = await DocumentsService.listDocuments();
```

The `NEXT_PUBLIC_API_URL` environment variable controls the base URL (default: `http://localhost:3001`).

## Tooling

| Tool | Config |
|---|---|
| `@hey-api/openapi-ts` | `packages/api-client/openapi-ts.config.ts` |
| `pnpm generate` script | `packages/api-client/package.json` |
