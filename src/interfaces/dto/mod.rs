//! Data Transfer Objects — JSON-serializable wire types for the HTTP API.
//!
//! DTOs are the only types that appear in the `Content-Type: application/json`
//! bodies that clients send and receive.  They are deliberately separate from
//! domain entities so that wire format changes can be made without touching
//! business logic.
//!
//! - [`request`]  — inbound DTO for `POST /api/2.0/sql/statements`
//! - [`response`] — outbound DTOs for all statement endpoints

pub mod request;
pub mod response;
