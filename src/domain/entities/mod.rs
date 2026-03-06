//! Domain entities — immutable value objects that form the vocabulary of the domain.
//!
//! - [`statement`] — `Statement`, `StatementResult`, `StatementState`, and related enums
//! - [`warehouse`] — `WarehouseConfig` describing an upstream Databricks warehouse

pub mod statement;
pub mod warehouse;
