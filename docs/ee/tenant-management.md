# Tenant Management

**Crate:** `crates/ee/kaya-tenant/`  
**License:** BSL 1.1  
**Status:** Not yet implemented

## Overview

`kaya-tenant` implements the multi-tenant organisation model used by the cloud distribution. Each tenant (organisation) is an isolated unit: documents, embeddings, sessions, and billing records are all scoped by `org_id`.

## Data model

```
Organisation
  └── Users (members of the org)
        └── Sessions (magic-link sessions per user)
        └── Spend records (per billing period)
        └── Documents (owned by org, not individual users)
              └── Chunks
              └── Embeddings
```

Organisations own their documents. Individual users within an org can read and edit documents according to their role (future: role-based access control).

## `UserContext`

`UserContext` is produced by `CloudAuthAdapter` after a successful session lookup:

```rust
pub struct UserContext {
    pub user_id: Uuid,
    pub org_id: Uuid,
}
```

It is passed into `PostgresAdapter::new(pool, user_ctx)` to scope all subsequent queries. Business logic never accesses `org_id` directly — the adapter enforces the boundary.

## Org provisioning

A new organisation is created when the first user signs up with a given email domain (configurable) or via an explicit invite flow. The provisioning process:

1. Creates an `organisations` row.
2. Creates the user row linked to the org.
3. Runs the per-org schema setup (RLS policies).
4. Creates the first Paddle subscription.

## Row-level security

All tables that contain tenant data have Postgres RLS policies enabled. The `PostgresAdapter` sets `app.current_org_id` on the session before executing any query:

```sql
SET LOCAL app.current_org_id = '<org_id>';
```

RLS policies filter all reads and writes to rows where `org_id = current_setting('app.current_org_id')::uuid`. This provides defence-in-depth on top of the application-level `UserContext` scoping.

## Invite flow (planned)

- Owner invites a new member by email.
- Resend delivers the invite link (same magic-link mechanism as regular auth).
- On acceptance, user row is created and linked to the org.
- Pending invites expire after 7 days.
