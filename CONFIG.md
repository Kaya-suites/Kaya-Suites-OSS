# Kaya Suites — Product Configuration Decisions

Decisions that were left as TBD in the BRD and have now been resolved. Do not change these
without re-running the cost model and updating pricing in `apps/backend/config/pricing.yaml`.

---

## D-12: Included monthly agent invocations

**Resolved: 50 invocations / user / month**

### Cost model

| Operation (per invocation)         | Model             | Est. tokens       | Est. cost   |
|------------------------------------|-------------------|-------------------|-------------|
| Edit proposal or document gen      | Claude Opus 4.6   | 2 000 in + 800 out | $0.090      |
| Retrieval classification (×1)      | GPT-4o-mini       | 500 in + 100 out  | $0.000090   |
| Stale detection (×1, when used)    | GPT-4o-mini       | 500 in + 100 out  | $0.000090   |
| Embeddings (incremental indexing)  | text-emb-3-small  | ~1 000            | $0.000020   |
| **Estimated cost per invocation**  |                   |                   | **~$0.090** |

50 invocations × $0.090 = **$4.50** of model costs per user per month.
Remaining $5.50 covers Neon DB ($2), Fly.io hosting ($1.50), Resend ($0.30), overhead.

Overages above 50 are billed at-cost via Paddle usage-based billing (D-5: zero margin).

---

## D-13: Storage cap per user

**Resolved: 1 GB per user**

| Content type            | Typical size                     | @ 1 GB cap   |
|-------------------------|----------------------------------|--------------|
| Markdown documents      | ~5 KB each                       | ~200 000 docs|
| pgvector embeddings     | 1 536 × 4 B = 6 KB/doc           | ~100 000 docs|
| Combined                | ~11 KB/doc                       | ~93 000 docs |

1 GB is extremely generous for a text knowledge base. No user is expected to approach it.
Neon charges ~$0.023/GB-hour; at 100 users × 0.1 GB average = $1.66/month total.

---

## D-14: Monthly model spend cap

**Resolved: $6.00 USD / user / month (hard cap)**

- **Soft alert at $4.80 (80%)** — email user + founder
- **Hard cap at $6.00 (100%)** — agent throttles; invocations return `SpendCapReached` error
- Covers: 50 included invocations ($4.50) + 16 burst invocations ($1.50 headroom)
- At-cost overages (FR-33) are billed via Paddle at period close if user consents to overage billing

---

## Global circuit breaker (BRD §12.5)

**Resolved: $50.00 USD aggregate daily spend threshold**

Expected daily aggregate at 100 active users: ~$15/day (100 users × 50 inv / 30 days × $0.09).
$50/day = 3.3× expected load. Triggers during anomalies (agent loop bugs, runaway sessions).

When tripped: new agent invocations are rejected with `CircuitBreakerOpen` error.
Founder alerted via Resend email. State persisted in `system_flags` table.

---

## Rate limits (FR-36)

| Window  | Token limit per user |
|---------|----------------------|
| Hourly  | 100 000 tokens       |
| Daily   | 500 000 tokens       |

100 K/hour ≈ 11 full Opus invocations or ~66 GPT-4o-mini calls. Prevents runaway loops.
500 K/day ≈ 55 Opus invocations. Generous enough not to throttle normal usage.

---

## Implementation references

- Token costs: `apps/backend/config/pricing.yaml`
- Enforcement code: `apps/backend/crates/ee/kaya-metering/`
- DB schema: `apps/backend/crates/ee/kaya-postgres-storage/migrations/004_metering.sql`
