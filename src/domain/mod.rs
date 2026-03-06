//! Domain layer — the innermost ring of the clean architecture.
//!
//! Contains the core business language of NiXQueryLink360:
//! - **entities** — value objects (`Statement`, `WarehouseConfig`, …)
//! - **errors**   — every failure mode expressed as a typed enum
//! - **ports**    — abstract interfaces (traits) that the application layer depends on;
//!                  concrete implementations live in `infrastructure`.
//!
//! This module has **zero** knowledge of HTTP, databases, or any external framework.

pub mod entities;
pub mod errors;
pub mod ports;
