# Contributing to Kaya Suites

Thank you for your interest in contributing.

## What is open source

Everything **outside** `apps/backend/crates/ee/` and `apps/backend/bin/kaya-cloud/` is Apache 2.0 and open to contributions. The `ee/` crates and the `kaya-cloud` binary are BSL 1.1 — source-available but not open to external contributions.

The OSS surface you can contribute to:
- `crates/kaya-core/` — core traits, edit primitives, agent loop
- `crates/kaya-storage/` — SQLite adapter
- `bin/kaya-oss/` — self-hosted binary
- `apps/web/app/(shared)/` — Apache-licensed Next.js pages

Never import an `ee/` crate from an Apache crate. The `bin/kaya-oss` binary must build with zero `ee/` dependencies.

## Development setup

```bash
# Frontend
pnpm install

# Backend
cd apps/backend && cargo build --workspace
```

### OSS static build (embeds frontend into binary)

```bash
cd apps/web
NEXT_PUBLIC_KAYA_BUILD=oss pnpm build
cp -r out ../backend/bin/kaya-oss/frontend
cd ../backend
cargo build --release --bin kaya-oss
```

## Running tests

```bash
# Rust
cd apps/backend && cargo test --workspace
# (ee/ integration tests skip without PG_TEST_DATABASE_URL)

# Frontend
pnpm --filter apps/web lint
```

## Submitting a PR

1. Fork the repo and create a branch from `main`.
2. Run `cargo test` and `pnpm build` before pushing.
3. Keep PRs focused — one feature or fix per PR.
4. Write tests for new behaviour.
5. Do not add BSL or SSPL dependencies to `bin/kaya-oss/Cargo.toml`.

CI must be green before merge.

## Code style

- Rust: `cargo fmt` and `cargo clippy --all-targets` must pass.
- TypeScript: `pnpm --filter apps/web lint` must pass.

## Licence

By contributing you agree your contribution is licensed under the same licence as the file being modified (Apache 2.0 for OSS files).
