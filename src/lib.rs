//! # NiXQueryLink360
//!
//! Alternative SQL Endpoint proxy for Databricks, built in Rust.
//!
//! ## Architecture (Clean Architecture)
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │  interfaces/   Axum HTTP handlers, DTOs,          │
//! │                auth middleware                    │
//! ├──────────────────────────────────────────────────┤
//! │  application/  Use cases — orchestrate domain     │
//! │                logic, no framework dependencies   │
//! ├──────────────────────────────────────────────────┤
//! │  infrastructure/  Databricks HTTP client,         │
//! │                   retry policy, settings loader   │
//! ├──────────────────────────────────────────────────┤
//! │  domain/       Entities, ports (traits), errors   │
//! │                — zero external dependencies       │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! ## Dependency Rule
//! Inner layers **must never** import from outer layers.
//! `domain` has zero third-party deps; `application` depends only on `domain`.
//!
//! ## Exposed Endpoints
//! | Method   | Path                                          | Auth     |
//! |----------|-----------------------------------------------|----------|
//! | `GET`    | `/health`                                     | None     |
//! | `GET`    | `/ready`                                      | None     |
//! | `POST`   | `/api/2.0/sql/statements`                     | Bearer   |
//! | `GET`    | `/api/2.0/sql/statements/{id}`                | Bearer   |
//! | `DELETE` | `/api/2.0/sql/statements/{id}/cancel`         | Bearer   |

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
