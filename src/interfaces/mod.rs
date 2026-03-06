//! Interfaces layer — translates between the HTTP world and the application/domain layers.
//!
//! This layer contains:
//! - **`dto`** — Data Transfer Objects for serializing/deserializing JSON bodies
//! - **`http`** — Axum router, middleware, and request handlers
//!
//! Handlers are intentionally thin: they extract data from HTTP requests,
//! build use-case input structs, delegate to the application layer, and map
//! domain results back to HTTP responses.  No business logic lives here.

pub mod dto;
pub mod http;
