---
id: 909d9c35-9fc1-4da2-bdea-8573a644fd0d
title: Engineering Onboarding Guide
last_reviewed: 2024-09-15
tags:
- engineering
- onboarding
- process
---

## Welcome

This guide covers everything you need to get started as an engineer at Kaya Suites. Read it top-to-bottom on your first day; the checklist at the end tracks your setup progress.

## Development Environment

All engineers use a MacBook with Apple Silicon (M-series). The standard toolchain is Homebrew, Rust (via rustup), Node.js 20 LTS (via nvm), and Docker Desktop. Request access to the 1Password team vault on day one — all shared secrets live there.

## Repository Structure

The monorepo lives at `github.com/kaya-suites/kaya`. The two primary build systems are:

- `apps/backend/` — Rust workspace (Cargo). Run `cargo build --workspace` to build everything.
- `apps/web/` — Next.js 16 (pnpm). Run `pnpm install && pnpm dev` from the repo root.

Never mix the two build systems. The only shared surface is `packages/api-client/`, a generated TypeScript client.

## Code Review Process

All changes require at least one approval from a senior engineer. PRs must pass CI (cargo test + pnpm build) before merging. We use conventional commits (`feat:`, `fix:`, `chore:`). Squash-merge only on the main branch.

## On-Call Rotation

Engineering rotates weekly on-call. The on-call engineer is responsible for monitoring Grafana, triaging PagerDuty alerts, and writing incident reports within 24 hours of any SEV-2 or higher event. The current rotation schedule is in Notion under "Engineering > On-Call".

## Key Contacts

- **CTO**: Priya Mehta — architecture decisions, escalations
- **EM**: Jordan Lee — sprint planning, resourcing
- **DevOps**: Alex Kimura — infra, deployments, secrets management
- **Security**: Dana Okonkwo — vulnerability reports, compliance reviews
