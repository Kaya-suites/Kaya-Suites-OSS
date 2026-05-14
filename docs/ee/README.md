# Kaya Suites — Enterprise Edition Documentation

**License: BSL 1.1**

This directory is stripped from the public OSS mirror before release. It contains documentation for cloud-only features that are not part of the Apache 2.0 distribution.

> Do not link to files in this directory from any Apache 2.0 documentation.

## Contents

| Document | Description |
|---|---|
| [Postgres Storage](postgres-storage.md) | `PostgresAdapter`, multi-tenancy, migrations |
| [Cloud Auth](cloud-auth.md) | `CloudAuthAdapter`, magic-link authentication |
| [Billing](billing.md) | Paddle integration, spend caps, rate limits |
| [Pricing Config](pricing-config.md) | Resolved configuration decisions and cost model |
| [Tenant Management](tenant-management.md) | Organisation and user model |

## Crate map

| Crate | Location | Purpose |
|---|---|---|
| `kaya-postgres-storage` | `crates/ee/kaya-postgres-storage/` | Postgres + pgvector `StorageAdapter` |
| `kaya-billing` | `crates/ee/kaya-billing/` | Paddle usage-based billing |
| `kaya-metering` | `crates/ee/kaya-metering/` | Per-user token and spend metering |
| `kaya-tenant` | `crates/ee/kaya-tenant/` | Multi-tenant organisation model |
| `kaya-cloud` binary | `bin/kaya-cloud/` | Cloud entrypoint; the only binary that links BSL crates |
