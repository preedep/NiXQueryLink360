//! Application use cases — one module per user-facing operation.
//!
//! | Module               | HTTP method | Description                               |
//! |----------------------|-------------|-------------------------------------------|
//! | [`submit_statement`] | `POST`      | Submit a SQL statement for execution      |
//! | [`get_statement`]    | `GET`       | Poll the state / result of a statement    |
//! | [`cancel_statement`] | `DELETE`    | Request cancellation of an in-flight statement |

pub mod submit_statement;
pub mod get_statement;
pub mod cancel_statement;
