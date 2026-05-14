# Pricing Configuration

**License:** BSL 1.1

Resolved configuration decisions for the cloud distribution. Do not change these values without re-running the cost model and updating `apps/backend/config/pricing.yaml`.

## Cost model per invocation

| Operation | Model | Estimated tokens | Estimated cost |
|---|---|---|---|
| Edit proposal or document generation | Claude Opus 4.6 | 2,000 in + 800 out | $0.090 |
| Retrieval classification | GPT-4o-mini | 500 in + 100 out | $0.000090 |
| Stale detection | GPT-4o-mini | 500 in + 100 out | $0.000090 |
| Embeddings (incremental indexing) | text-embedding-3-small | ~1,000 | $0.000020 |
| **Total per invocation** | | | **~$0.090** |

## Monthly plan economics (per user)

| Line item | Amount |
|---|---|
| 50 invocations × $0.090 model cost | $4.50 |
| Neon DB (shared, per-user allocation) | $2.00 |
| Fly.io hosting (per-user allocation) | $1.50 |
| Resend transactional email | $0.30 |
| **Total monthly cost** | **$8.30** |

The plan price must cover $8.30/user/month plus payment-processor fees.

## Storage economics

| Content type | Typical size | At 1 GB cap |
|---|---|---|
| Markdown documents | ~5 KB each | ~200,000 docs |
| pgvector embeddings (1,536 × 4 B) | ~6 KB/doc | ~100,000 docs |
| Combined | ~11 KB/doc | ~93,000 docs |

Neon charges ~$0.023/GB-hour. At 100 users × 0.1 GB average: $1.66/month total.

## Spend cap configuration

| Threshold | Value | Action |
|---|---|---|
| Soft alert | $4.80 (80%) | Email user + founder |
| Hard cap | $6.00 (100%) | Block invocations (`SpendCapReached`) |
| Burst headroom above included cost | $1.50 | ~16 additional invocations |
| Global circuit breaker | $50.00/day aggregate | Block all users, alert founder |

## Rate limit configuration

| Window | Token limit per user |
|---|---|
| Hourly | 100,000 tokens |
| Daily | 500,000 tokens |

100K/hour ≈ 11 full Opus invocations or ~66 GPT-4o-mini calls.  
500K/day ≈ 55 Opus invocations — generous enough not to throttle normal usage.

## Pricing YAML

Actual per-token costs are read at runtime from `apps/backend/config/pricing.yaml`. This file is the authoritative source for cost calculations in `kaya-metering`. Update it when model pricing changes, then re-run the cost model above.
