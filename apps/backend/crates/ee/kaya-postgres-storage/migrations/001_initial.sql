-- Kaya Suites cloud schema — BSL 1.1
--
-- Every user-scoped table carries a NOT NULL user_id column with a foreign-key
-- constraint back to `users`. This is the schema-level enforcement of NFR §6.3
-- (multi-tenant data isolation). The application-level enforcement is provided
-- by PostgresAdapter, whose constructor requires a UserContext and whose query
-- methods unconditionally filter by self.user_context.user_id.
--
-- Run once against a fresh Neon database, or re-run safely (all DDL is
-- guarded with IF NOT EXISTS / OR REPLACE).

-- pgvector: required for VECTOR columns and the <=> cosine-distance operator.
CREATE EXTENSION IF NOT EXISTS vector;

-- ── users ──────────────────────────────────────────────────────────────────────
-- Anchor table for all user-scoped rows. Rows are inserted by the auth layer
-- (CloudAuthAdapter, next prompt). The storage adapter never creates users.
CREATE TABLE IF NOT EXISTS users (
    id         UUID        NOT NULL DEFAULT gen_random_uuid(),
    email      TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    UNIQUE (email)
);

-- ── documents ──────────────────────────────────────────────────────────────────
-- Cloud-native document store. Source of truth is this table (not disk files,
-- unlike the SQLite adapter). The body column holds raw Markdown.
CREATE TABLE IF NOT EXISTS documents (
    id               UUID        NOT NULL DEFAULT gen_random_uuid(),
    user_id          UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title            TEXT        NOT NULL,
    owner            TEXT,
    last_reviewed    DATE,
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    related_docs     UUID[]      NOT NULL DEFAULT '{}',
    body             TEXT        NOT NULL DEFAULT '',
    content_hash     TEXT        NOT NULL DEFAULT '',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at       TIMESTAMPTZ,
    PRIMARY KEY (id)
);
-- Supports list_documents (WHERE user_id = $1 AND deleted_at IS NULL ORDER BY updated_at DESC).
CREATE INDEX IF NOT EXISTS documents_user_active
    ON documents (user_id, updated_at DESC)
    WHERE deleted_at IS NULL;

-- ── document_versions ──────────────────────────────────────────────────────────
-- Append-only edit history written by the approval workflow (FR-14 / FR-15).
-- The storage adapter does not write here; it will be used by the agent loop.
CREATE TABLE IF NOT EXISTS document_versions (
    id            UUID        NOT NULL DEFAULT gen_random_uuid(),
    document_id   UUID        NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    user_id       UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    body_snapshot TEXT        NOT NULL,
    content_hash  TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS document_versions_doc
    ON document_versions (user_id, document_id, created_at DESC);

-- ── chunks ─────────────────────────────────────────────────────────────────────
-- Paragraph-level metadata table and BM25 full-text index via Postgres FTS.
-- The `tsv` column is a STORED generated column so queries never re-tokenise.
CREATE TABLE IF NOT EXISTS chunks (
    user_id      UUID    NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    document_id  UUID    NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    paragraph_id TEXT    NOT NULL,
    ordinal      INTEGER NOT NULL,
    content      TEXT    NOT NULL,
    content_hash TEXT    NOT NULL,
    tsv          TSVECTOR GENERATED ALWAYS AS (to_tsvector('english', content)) STORED,
    PRIMARY KEY (user_id, document_id, paragraph_id)
);
-- GIN index for the FTS queries in search_text.
CREATE INDEX IF NOT EXISTS chunks_tsv ON chunks USING GIN (tsv);

-- ── chunk_embeddings ───────────────────────────────────────────────────────────
-- pgvector embeddings for semantic search (FR-7).
-- Dimension 1536 matches OpenAI text-embedding-3-small and Voyage-3-lite.
-- The HNSW index supports sub-millisecond ANN even on an empty table (unlike
-- IVFFlat, which requires a training pass over existing data).
CREATE TABLE IF NOT EXISTS chunk_embeddings (
    user_id      UUID         NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    document_id  UUID         NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    paragraph_id TEXT         NOT NULL,
    vector       VECTOR(1536) NOT NULL,
    PRIMARY KEY (user_id, document_id, paragraph_id)
);
CREATE INDEX IF NOT EXISTS chunk_embeddings_hnsw
    ON chunk_embeddings USING hnsw (vector vector_cosine_ops);

-- ── chat_sessions ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS chat_sessions (
    id         UUID        NOT NULL DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS chat_sessions_user
    ON chat_sessions (user_id, updated_at DESC);

-- ── chat_messages ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS chat_messages (
    id         UUID        NOT NULL DEFAULT gen_random_uuid(),
    session_id UUID        NOT NULL REFERENCES chat_sessions (id) ON DELETE CASCADE,
    user_id    UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role       TEXT        NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content    TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS chat_messages_session
    ON chat_messages (session_id, user_id, created_at ASC);

-- ── tool_invocations ───────────────────────────────────────────────────────────
-- Agent loop audit log — one row per tool call (FR-14).
CREATE TABLE IF NOT EXISTS tool_invocations (
    id          UUID        NOT NULL DEFAULT gen_random_uuid(),
    session_id  UUID        NOT NULL REFERENCES chat_sessions (id) ON DELETE CASCADE,
    user_id     UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    tool_name   TEXT        NOT NULL,
    input_json  JSONB       NOT NULL,
    output_json JSONB,
    started_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS tool_invocations_session
    ON tool_invocations (session_id, user_id);

-- ── usage_counters ─────────────────────────────────────────────────────────────
-- Per-user, per-period token and embedding-call metering (NFR §6.2).
-- Written by kaya-metering; read by kaya-billing.
CREATE TABLE IF NOT EXISTS usage_counters (
    id           UUID   NOT NULL DEFAULT gen_random_uuid(),
    user_id      UUID   NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    period_start DATE   NOT NULL,
    tokens_in    BIGINT NOT NULL DEFAULT 0,
    tokens_out   BIGINT NOT NULL DEFAULT 0,
    embed_calls  BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    UNIQUE (user_id, period_start)
);

-- ── subscriptions ──────────────────────────────────────────────────────────────
-- Stripe subscription state. At most one subscription per user (UNIQUE index).
CREATE TABLE IF NOT EXISTS subscriptions (
    id                 UUID        NOT NULL DEFAULT gen_random_uuid(),
    user_id            UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    stripe_sub_id      TEXT        UNIQUE,
    status             TEXT        NOT NULL DEFAULT 'trialing'
                           CHECK (status IN ('trialing', 'active', 'past_due', 'canceled', 'incomplete')),
    current_period_end TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);
CREATE UNIQUE INDEX IF NOT EXISTS subscriptions_user ON subscriptions (user_id);
