# AuthAdapter

**Trait location:** `crates/kaya-core/src/auth.rs`  
**License:** Apache 2.0

## Purpose

`AuthAdapter` decouples authentication from business logic, allowing the OSS binary to ship without any network-based auth while the cloud binary uses magic-link sessions.

## Types

### `UserSession`

```rust
pub struct UserSession {
    pub user_id: Uuid,
}
```

Represents a successfully authenticated user. Passed into business logic that requires an identity (e.g. metering, audit log).

## Trait methods

```rust
#[async_trait]
pub trait AuthAdapter: Send + Sync {
    async fn current_user(&self) -> Result<Option<UserSession>, KayaError>;
    async fn require_auth(&self) -> Result<UserSession, KayaError>;
}
```

| Method | Behaviour |
|---|---|
| `current_user` | Returns the session if the request is authenticated, or `None`. Never errors for unauthenticated requests. |
| `require_auth` | Returns the session, or `Err(KayaError::Unauthenticated)` if no valid credentials are present. Use this in handlers that must be protected. |

## Implementations

### `LocalAuthAdapter` (Apache 2.0) — not yet implemented

Returns a fixed single-user session with a deterministic UUID. Performs no network call. Used by `bin/kaya-oss`.

### `CloudAuthAdapter` (BSL 1.1) — not yet implemented

Validates a session cookie against the database. Used by `bin/kaya-cloud`. See `docs/ee/cloud-auth.md`.

## Usage pattern

```rust
async fn handle_edit(
    auth: Arc<dyn AuthAdapter>,
    storage: Arc<dyn StorageAdapter>,
    …
) -> Result<…> {
    let session = auth.require_auth().await?;
    // session.user_id is now available for metering / audit
}
```
