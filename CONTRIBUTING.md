# Contributing

## License boundary

- Files outside `ee/` directories: **Apache 2.0**. Contributions are welcome.
- Files inside `ee/` directories: **BSL 1.1**. Not accepted from external contributors.

Never import an `ee/` crate from an Apache crate. The `bin/kaya-oss` binary must build with zero `ee/` dependencies.

## Development setup

```bash
# Frontend
pnpm install

# Backend
cd apps/backend && cargo build --workspace
```

## Running tests

```bash
# Rust
cd apps/backend && cargo test --workspace

# Frontend
pnpm --filter apps/web lint
```

## Submitting changes

Open a PR against `main`. CI must be green before merge.
