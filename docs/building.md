# Building

## Prerequisites

- Rust (stable toolchain, edition 2024 crates)
- Node.js + pnpm
- `sqlite3` system library (for `kaya-storage`)

## Frontend

```bash
# Install dependencies (run from repo root)
pnpm install

# Start the dev server
pnpm dev

# Production build
pnpm --filter apps/web build
```

The frontend dev server proxies API calls to `NEXT_PUBLIC_API_URL` (default `http://localhost:3001`).

## Backend

```bash
cd apps/backend

cargo build --workspace

# Run the OSS binary
cargo run --bin kaya-oss

cargo test --workspace
```

## OSS static binary (embeds frontend)

The `kaya-oss` binary can serve the frontend directly without a separate Node.js process. This is the recommended distribution for self-hosted deployments.

```bash
# 1. Build the frontend in OSS mode
cd apps/web
NEXT_PUBLIC_KAYA_BUILD=oss pnpm build

# 2. Copy the static output into the binary's asset directory
cp -r out ../backend/bin/kaya-oss/frontend

# 3. Build the release binary
cd ../backend
cargo build --release --bin kaya-oss
```

The resulting binary at `apps/backend/target/release/kaya-oss` is fully self-contained.

## Code quality

```bash
# Rust
cd apps/backend
cargo fmt
cargo clippy --all-targets

# TypeScript / Next.js
pnpm --filter apps/web lint
```

Both must pass before a PR can be merged.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `NEXT_PUBLIC_API_URL` | `http://localhost:3001` | Backend base URL used by the frontend |
| `ANTHROPIC_API_KEY` | — | Required for `DocumentGeneration` and `EditProposal` operations |
| `OPENAI_API_KEY` | — | Required for classification, stale detection, and embedding |

Provider API keys are read from the environment variables named in `kaya.yaml` (see [LLM Provider](llm-provider.md)).
