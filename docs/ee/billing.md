# Billing

**Crate:** `crates/ee/kaya-billing/`  
**License:** BSL 1.1

## Overview

Billing is handled through [Paddle](https://paddle.com) using usage-based billing. Each user receives 50 included agent invocations per month; overages above that are billed at cost (zero margin) via Paddle's usage-based billing API.

## Included plan

| Item | Value |
|---|---|
| Included invocations / user / month | 50 |
| Storage cap / user | 1 GB |
| Monthly model spend cap | $6.00 USD |
| Overage billing | At cost via Paddle |

## Spend enforcement

Enforcement is implemented in `crates/ee/kaya-metering/` and enforced on every agent invocation:

| Threshold | Event |
|---|---|
| 80% of $6.00 ($4.80) | Soft alert — email sent to user and founder via Resend |
| 100% of $6.00 ($6.00) | Hard cap — invocations return `SpendCapReached`; agent is throttled |

The spend cap state is stored in the `user_spend` table and reset at the start of each billing period.

## Global circuit breaker

A global circuit breaker prevents runaway spend across all users (e.g. from an agent loop bug):

| Parameter | Value |
|---|---|
| Daily aggregate threshold | $50.00 USD |
| Trigger action | New invocations rejected with `CircuitBreakerOpen` |
| Alert | Founder email via Resend |
| Reset | Manual via `kaya-cloud admin circuit-breaker reset` |

State is persisted in the `system_flags` table.

## Rate limits

Rate limits prevent runaway token consumption per user regardless of spend:

| Window | Token limit per user |
|---|---|
| Hourly | 100,000 tokens |
| Daily | 500,000 tokens |

Limits are enforced by `kaya-metering` before each LLM call and reset on the UTC hour/day boundary.

## Paddle integration

- Subscriptions and usage records are managed via the Paddle API.
- At-cost overages are reported to Paddle at period close (not in real time).
- Users must opt in to overage billing; without consent, invocations are blocked after the 50-invocation included limit.

## Implementation references

- Token costs: `apps/backend/config/pricing.yaml`
- Enforcement code: `crates/ee/kaya-metering/`
- DB schema: `crates/ee/kaya-postgres-storage/migrations/004_metering.sql`

## Environment variables

| Variable | Description |
|---|---|
| `PADDLE_API_KEY` | Paddle vendor API key |
| `PADDLE_WEBHOOK_SECRET` | Secret for validating Paddle webhook signatures |
| `RESEND_API_KEY` | Used for soft-cap and circuit-breaker alert emails |
