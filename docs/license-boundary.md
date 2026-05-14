# License Boundary

Kaya Suites uses two licenses separated by a directory convention:

| Path pattern | License | Binary |
|---|---|---|
| Everything outside `ee/` | Apache 2.0 | `kaya-oss` |
| Any `ee/` directory | BSL 1.1 | `kaya-cloud` only |

## The `ee/` convention

Any directory named `ee/` is BSL 1.1, regardless of nesting depth:

```
apps/backend/crates/ee/          ← BSL 1.1 Rust crates
apps/web/app/(ee)/               ← BSL 1.1 Next.js routes
apps/web/components/ee/          ← BSL 1.1 React components
docs/ee/                         ← BSL 1.1 documentation
```

The `strip-ee.sh` script removes every directory named `ee/` before syncing to the public mirror.

## License files

| File | License |
|---|---|
| `LICENSE` | Apache 2.0 — applies to everything outside `ee/` |
| `LICENSE-BSL` | BSL 1.1 — applies to all `ee/` content; removed from OSS mirror |

## Rules for contributors

- **Never import** an `ee/` crate from an Apache 2.0 crate. The compiler will allow it locally but CI runs `cargo build --bin kaya-oss` without the EE crates present, which will catch the violation.
- `bin/kaya-oss/Cargo.toml` must list **only** Apache 2.0 crates as dependencies.
- `bin/kaya-cloud/Cargo.toml` is the **only** place BSL crates are pulled in.
- Do not add BSL or SSPL third-party dependencies to any Apache 2.0 crate.

## OSS mirror sync

The release workflow (`scripts/strip-ee.sh`, called on tag push) performs these steps:

1. Deletes every `ee/` directory anywhere in the tree.
2. Removes `apps/web/app/(ee)/` (the Next.js route group).
3. Removes `bin/kaya-cloud/`.
4. Removes `LICENSE-BSL`.
5. Strips BSL workspace members from `apps/backend/Cargo.toml`.
6. Verifies no remaining `Cargo.toml` references a deleted BSL crate.

The resulting tree is what appears in the public mirror and is fully buildable as `cargo build --bin kaya-oss`.
