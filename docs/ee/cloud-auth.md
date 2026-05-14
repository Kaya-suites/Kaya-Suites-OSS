# Cloud Auth

**Crate:** `crates/ee/kaya-tenant/` (session handling) + `crates/ee/kaya-postgres-storage/` (session table)  
**License:** BSL 1.1  
**Status:** Not yet implemented (trait only in `kaya-core`)

## Overview

`CloudAuthAdapter` is the BSL 1.1 implementation of `AuthAdapter`. It validates a session cookie against a database-backed session table and returns a scoped `UserSession` that includes the user's `org_id` for multi-tenant query scoping.

## Authentication flow

```
1. User submits email → POST /auth/magic-link
2. kaya-cloud generates a signed, time-limited token and emails it via Resend
3. User clicks the link → GET /auth/verify?token=…
4. kaya-cloud validates token, creates a session row, sets a secure HttpOnly cookie
5. Subsequent requests carry the cookie → CloudAuthAdapter.require_auth() → UserSession
```

## Session cookie

- Name: `kaya_session`
- HttpOnly, SameSite=Strict, Secure (HTTPS only in production)
- Lifetime: 30 days, sliding window on each authenticated request
- Session row is invalidated on explicit sign-out (`DELETE /auth/session`)

## `CloudAuthAdapter` methods

| Method | Behaviour |
|---|---|
| `current_user` | Reads the `kaya_session` cookie, looks up the session in Postgres. Returns `None` for missing or expired sessions. |
| `require_auth` | Same as `current_user` but returns `Err(KayaError::Unauthenticated)` instead of `None`. Use in all protected handlers. |

## `UserSession` (cloud extension)

The cloud binary extends the Apache 2.0 `UserSession` with an `org_id` field used to scope `PostgresAdapter` queries. The extension is not visible to `kaya-core` — it is carried through the `CloudAuthAdapter` and injected into the adapter at construction time.

## Magic-link email

Emails are sent via the [Resend](https://resend.com) API. The Resend API key is read from the `RESEND_API_KEY` environment variable. See [Billing](billing.md) for the Resend spend allocation.

## Environment variables

| Variable | Description |
|---|---|
| `SESSION_SECRET` | 32-byte hex secret used to sign session tokens |
| `RESEND_API_KEY` | API key for transactional email (magic links, alerts) |
| `MAGIC_LINK_TTL_MINUTES` | Token lifetime before the link expires (default: 15) |
