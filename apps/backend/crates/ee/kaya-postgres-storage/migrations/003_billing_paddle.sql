-- Migrate subscriptions table from Stripe model to Paddle (BSL 1.1)
--
-- Changes:
--   1. Rename stripe_sub_id → paddle_subscription_id.
--   2. Add paddle_customer_id and current_period_start columns.
--   3. Replace the Stripe-era status constraint with Paddle's lifecycle states:
--      active | grace_period | cancelled | refunded.
--   4. Add lookup indexes for Paddle IDs.

-- Drop the old inline UNIQUE constraint created with stripe_sub_id.
ALTER TABLE subscriptions
    DROP CONSTRAINT IF EXISTS subscriptions_stripe_sub_id_key;

-- Rename to Paddle vocabulary.
ALTER TABLE subscriptions
    RENAME COLUMN stripe_sub_id TO paddle_subscription_id;

-- Add Paddle-specific columns (IF NOT EXISTS guards re-entrancy).
ALTER TABLE subscriptions
    ADD COLUMN IF NOT EXISTS paddle_customer_id   TEXT,
    ADD COLUMN IF NOT EXISTS current_period_start TIMESTAMPTZ;

-- Drop the old Stripe-era status constraint and default before redefining them.
ALTER TABLE subscriptions
    ALTER COLUMN status DROP DEFAULT;

ALTER TABLE subscriptions
    DROP CONSTRAINT IF EXISTS subscriptions_status_check;

-- Map any existing rows to the new status vocabulary so the new CHECK will pass.
UPDATE subscriptions
SET status = CASE
    WHEN status IN ('trialing', 'active', 'incomplete') THEN 'active'
    WHEN status = 'past_due'                             THEN 'grace_period'
    WHEN status IN ('canceled', 'cancelled')             THEN 'cancelled'
    ELSE 'active'
END;

-- Add the Paddle-era constraint.
ALTER TABLE subscriptions
    ADD CONSTRAINT subscriptions_status_check
    CHECK (status IN ('active', 'grace_period', 'cancelled', 'refunded'));

ALTER TABLE subscriptions
    ALTER COLUMN status SET DEFAULT 'active';

-- Partial unique index: NULL values are excluded, so un-provisioned rows do not
-- conflict.  ON CONFLICT clauses in INSERT statements must repeat the WHERE
-- clause to match this index.
CREATE UNIQUE INDEX IF NOT EXISTS subscriptions_paddle_sub_idx
    ON subscriptions (paddle_subscription_id)
    WHERE paddle_subscription_id IS NOT NULL;

-- Supports webhook lookups by Paddle customer ID.
CREATE INDEX IF NOT EXISTS subscriptions_paddle_customer_idx
    ON subscriptions (paddle_customer_id)
    WHERE paddle_customer_id IS NOT NULL;
