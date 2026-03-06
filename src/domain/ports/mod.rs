//! Ports — abstract outbound interfaces defined by the domain.
//!
//! A *port* is a trait that the domain and application layers depend on.
//! Infrastructure adapters implement these traits; the application never
//! imports concrete implementations directly (Dependency Inversion Principle).
//!
//! - [`warehouse_client`] — trait for submitting, polling, and cancelling
//!   SQL statements against an upstream Databricks warehouse.

pub mod warehouse_client;
