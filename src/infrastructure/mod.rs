//! Infrastructure layer — concrete implementations of domain ports and cross-cutting concerns.
//!
//! This layer is the **only** place where external dependencies (HTTP clients,
//! config files, environment variables) are imported.  It depends on `domain`
//! (to implement port traits) but is **never** imported by `domain` or `application`.
//!
//! - [`config`]      — TOML + environment variable configuration loading
//! - [`http_client`] — `reqwest`-based Databricks REST client with retry logic

pub mod config;
pub mod http_client;
