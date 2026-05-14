# Kaya Suites — Documentation

This directory contains documentation for the Apache 2.0 open-source distribution of Kaya Suites.

> Enterprise / cloud documentation lives in `docs/ee/` and is stripped from the public mirror before release.

## Contents

| Document | Description |
|---|---|
| [Architecture](architecture.md) | System overview, two-build-system layout, dependency graph |
| [Storage Adapter](storage-adapter.md) | `StorageAdapter` trait, domain types, `SqliteAdapter` |
| [Auth Adapter](auth-adapter.md) | `AuthAdapter` trait and `LocalAuthAdapter` (single-user) |
| [LLM Provider](llm-provider.md) | `LlmProvider` trait, `ModelRouter`, routing config |
| [API Codegen](api-codegen.md) | OpenAPI schema → TypeScript client pipeline |
| [Building](building.md) | Building frontend, backend, and the OSS static binary |
| [License Boundary](license-boundary.md) | How Apache 2.0 and BSL 1.1 are separated |
