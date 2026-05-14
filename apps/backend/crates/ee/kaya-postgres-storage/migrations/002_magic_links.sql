-- Magic-link authentication tokens (BSL 1.1)
--
-- Tokens are single-use with a 15-minute TTL (FR-28).
-- The raw token is never stored; only SHA-256(token) is persisted so a
-- database breach cannot be replayed against the auth endpoint.
--
-- `email` is NOT a FK into `users` because a magic-link can be requested
-- before the first sign-in (i.e. user row may not yet exist). The verify
-- handler upserts the user row on successful token redemption.

CREATE TABLE IF NOT EXISTS magic_links (
    id         UUID        NOT NULL DEFAULT gen_random_uuid(),
    email      TEXT        NOT NULL,
    token_hash TEXT        NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    UNIQUE (token_hash)
);

-- Partial index: only un-used tokens are looked up during verification.
CREATE INDEX IF NOT EXISTS magic_links_pending
    ON magic_links (token_hash, expires_at)
    WHERE used_at IS NULL;

-- Housekeeping: clean up tokens older than 1 day.
-- (The application layer rejects expired tokens; this index supports
-- a pg_cron job or periodic vacuum, not application queries.)
CREATE INDEX IF NOT EXISTS magic_links_expires ON magic_links (expires_at);
