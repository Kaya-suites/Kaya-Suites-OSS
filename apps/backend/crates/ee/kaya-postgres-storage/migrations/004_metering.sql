-- Metering tables for Kaya Suites cloud (BSL 1.1)
--
-- usage_events      — one row per LLM call; the raw audit log.
-- rate_limit_windows — hourly and daily token buckets per user (Postgres-backed,
--                      no Redis dependency in v0).
-- system_flags       — key/value store for persisted operational state
--                      (circuit breaker, maintenance mode, etc.).
--
-- Also extends usage_counters (from migration 001) with an agent_invocations
-- column for period-level rollups.

-- ── usage_events ───────────────────────────────────────────────────────────────
-- Raw event log.  One row per LLM call.  Never updated; only inserted.
-- Queries for the current period filter on recorded_at >= date_trunc('month', now()).
CREATE TABLE IF NOT EXISTS usage_events (
    id            UUID             NOT NULL DEFAULT gen_random_uuid(),
    user_id       UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    operation     TEXT             NOT NULL,
    model         TEXT             NOT NULL,
    input_tokens  INTEGER          NOT NULL,
    output_tokens INTEGER          NOT NULL,
    cost_usd      DOUBLE PRECISION NOT NULL,
    recorded_at   TIMESTAMPTZ      NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
-- Supports current-period spend queries per user.
CREATE INDEX IF NOT EXISTS usage_events_user_period
    ON usage_events (user_id, recorded_at DESC);
-- Supports global circuit-breaker daily aggregation.
CREATE INDEX IF NOT EXISTS usage_events_daily
    ON usage_events (recorded_at DESC);

-- ── rate_limit_windows ─────────────────────────────────────────────────────────
-- Token buckets for hourly and daily rate limits (FR-36).
-- window_start is truncated to the hour (hourly) or midnight (daily).
-- Rows are cheap to create and accumulate; a pg_cron job or periodic task
-- should purge windows older than 7 days.
CREATE TABLE IF NOT EXISTS rate_limit_windows (
    user_id      UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    window_type  TEXT        NOT NULL CHECK (window_type IN ('hourly', 'daily')),
    window_start TIMESTAMPTZ NOT NULL,
    tokens_used  BIGINT      NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, window_type, window_start)
);

-- ── system_flags ───────────────────────────────────────────────────────────────
-- Persisted operational flags (circuit breaker, etc.).
CREATE TABLE IF NOT EXISTS system_flags (
    key        TEXT        NOT NULL,
    value      TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (key)
);

-- ── usage_counters extension ───────────────────────────────────────────────────
-- Add agent_invocations to the existing monthly rollup table.
ALTER TABLE usage_counters
    ADD COLUMN IF NOT EXISTS agent_invocations BIGINT NOT NULL DEFAULT 0;
