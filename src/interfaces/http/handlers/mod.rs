//! Axum request handlers for all NiXQueryLink360 HTTP endpoints.
//!
//! - [`health`]     — liveness (`GET /health`) and readiness (`GET /ready`) probes
//! - [`statements`] — SQL statement lifecycle: submit, poll, cancel

pub mod health;
pub mod statements;
