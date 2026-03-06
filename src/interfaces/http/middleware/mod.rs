//! Axum middleware for the NiXQueryLink360 HTTP server.
//!
//! - [`auth`] — Bearer token extraction and injection into request extensions.
//!   All SQL API routes are protected by this middleware.

pub mod auth;
