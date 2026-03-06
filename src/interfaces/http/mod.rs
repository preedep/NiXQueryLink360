//! Axum HTTP interface — router, middleware, and request handlers.
//!
//! - [`router`]     — wires all routes, middleware layers, and application state
//! - [`handlers`]   — thin async functions that translate HTTP ↔ use-case types
//! - [`middleware`] — reusable Axum middleware (Bearer token auth, …)

pub mod handlers;
pub mod middleware;
pub mod router;
