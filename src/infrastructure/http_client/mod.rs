//! HTTP client infrastructure for the upstream Databricks REST API.
//!
//! - [`databricks_client`] — implements [`crate::domain::ports::warehouse_client::WarehouseClient`]
//!   using `reqwest`, with connection pooling and per-request retry support.
//! - [`retry`] — generic exponential-backoff retry policy used by the client.

pub mod databricks_client;
pub mod retry;
