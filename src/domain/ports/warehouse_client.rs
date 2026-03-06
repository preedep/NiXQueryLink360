//! Port: abstract interface for communicating with an upstream SQL warehouse.
//!
//! The domain and application layers depend only on this trait.
//! The concrete implementation ([`crate::infrastructure::http_client::databricks_client::DatabricksClient`])
//! lives in the infrastructure layer and is wired at startup via dependency injection.

use async_trait::async_trait;
use crate::domain::{
    entities::statement::{Statement, StatementResult},
    errors::DomainError,
};

/// Outbound port for executing SQL statements against an upstream warehouse.
///
/// Any infrastructure adapter that can submit, poll, and cancel statements
/// must implement this trait.  The application layer calls only the methods
/// declared here — it never imports a concrete HTTP client or database driver.
///
/// # Thread-safety
/// Implementations must be [`Send`] + [`Sync`] so they can be shared across
/// Tokio tasks via [`std::sync::Arc`].
#[async_trait]
pub trait WarehouseClient: Send + Sync {
    /// Submit a SQL statement to the upstream warehouse for execution.
    ///
    /// The statement may be executed synchronously (if `wait_timeout_secs > 0`
    /// and the query finishes in time) or asynchronously (returning a statement
    /// ID that the caller must poll).
    ///
    /// # Parameters
    /// - `statement` — fully-validated domain entity ready for dispatch
    /// - `token`     — caller-supplied Bearer token (PAT or OAuth)
    ///
    /// # Errors
    /// - [`DomainError::AuthenticationFailed`] — token rejected by upstream
    /// - [`DomainError::UpstreamError`]        — non-2xx response or network failure
    async fn submit_statement(
        &self,
        statement: &Statement,
        token: &str,
    ) -> Result<StatementResult, DomainError>;

    /// Poll the current state and result of a previously submitted statement.
    ///
    /// Clients using the async pattern call this repeatedly until `state` is
    /// one of `SUCCEEDED`, `FAILED`, or `CANCELLED`.
    ///
    /// # Parameters
    /// - `statement_id` — Databricks-assigned statement identifier
    /// - `warehouse_id` — warehouse the statement was submitted to (for routing)
    /// - `token`        — caller-supplied Bearer token
    ///
    /// # Errors
    /// - [`DomainError::StatementNotFound`] — the ID does not exist upstream
    /// - [`DomainError::UpstreamError`]     — unexpected non-2xx response
    async fn get_statement(
        &self,
        statement_id: &str,
        warehouse_id: &str,
        token: &str,
    ) -> Result<StatementResult, DomainError>;

    /// Request cancellation of a statement that is `PENDING` or `RUNNING`.
    ///
    /// Cancellation is best-effort: if the statement has already reached a
    /// terminal state (`SUCCEEDED`, `FAILED`, `CANCELLED`) the upstream may
    /// return an error, which is propagated as [`DomainError::UpstreamError`].
    ///
    /// # Parameters
    /// - `statement_id` — Databricks-assigned statement identifier
    /// - `warehouse_id` — warehouse the statement was submitted to (for routing)
    /// - `token`        — caller-supplied Bearer token
    ///
    /// # Errors
    /// - [`DomainError::UpstreamError`] — cancellation rejected or network failure
    async fn cancel_statement(
        &self,
        statement_id: &str,
        warehouse_id: &str,
        token: &str,
    ) -> Result<(), DomainError>;
}
