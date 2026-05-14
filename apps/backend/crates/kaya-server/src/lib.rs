pub mod error;
pub mod routes;
pub mod state;

pub use routes::router;

/// Re-export so callers can call `kaya_server::router::<S>()`.
pub use axum::Router;
