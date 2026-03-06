//! Domain-level error types.
//!
//! All errors originate here and are mapped to HTTP status codes
//! at the interface layer — keeping HTTP concerns out of domain logic.

use thiserror::Error;

/// All possible failures within the domain.
///
/// Each variant maps to a specific HTTP status code via [`DomainError::http_status_code`].
/// Infrastructure adapters translate their own errors into these variants before
/// returning them to the application layer.
#[derive(Debug, Error)]
pub enum DomainError {
    /// The requested `statement_id` does not exist in the upstream warehouse.
    /// Maps to HTTP 404.
    #[error("Statement not found: {statement_id}")]
    StatementNotFound { statement_id: String },

    /// The caller provided a request that fails validation (empty SQL, bad timeout, etc.).
    /// Maps to HTTP 400.
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// The upstream Databricks API returned an unexpected error.
    /// Includes raw HTTP status and body for diagnostics.
    /// Maps to HTTP 502 (Bad Gateway).
    #[error("Upstream error: {message}")]
    UpstreamError { message: String },

    /// The provided Bearer token was rejected by the upstream.
    /// Maps to HTTP 401.
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    /// No warehouse configuration found for the given `warehouse_id`.
    /// Check `config.toml` → `[[upstream.warehouses]]`.
    /// Maps to HTTP 404.
    #[error("Warehouse not found: {warehouse_id}")]
    WarehouseNotFound { warehouse_id: String },

    /// The statement was successfully cancelled before it completed.
    /// Maps to HTTP 200.
    #[error("Statement cancelled")]
    StatementCancelled,

    /// The upstream did not respond within the configured timeout window.
    /// Maps to HTTP 408.
    #[error("Request timeout")]
    Timeout,
}

impl DomainError {
    /// Returns the HTTP status code that best represents this error,
    /// used by the interface layer when building error responses.
    pub fn http_status_code(&self) -> u16 {
        match self {
            DomainError::StatementNotFound { .. }    => 404,
            DomainError::InvalidRequest { .. }       => 400,
            DomainError::AuthenticationFailed { .. } => 401,
            DomainError::WarehouseNotFound { .. }    => 404,
            DomainError::UpstreamError { .. }        => 502,
            DomainError::StatementCancelled          => 200,
            DomainError::Timeout                     => 408,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statement_not_found_message() {
        let err = DomainError::StatementNotFound {
            statement_id: "abc-123".to_string(),
        };
        assert!(err.to_string().contains("abc-123"));
        assert_eq!(err.http_status_code(), 404);
    }

    #[test]
    fn test_invalid_request_status() {
        let err = DomainError::InvalidRequest {
            message: "bad input".to_string(),
        };
        assert_eq!(err.http_status_code(), 400);
    }

    #[test]
    fn test_auth_failed_status() {
        let err = DomainError::AuthenticationFailed {
            message: "no token".to_string(),
        };
        assert_eq!(err.http_status_code(), 401);
    }

    #[test]
    fn test_upstream_error_status() {
        let err = DomainError::UpstreamError {
            message: "connection refused".to_string(),
        };
        assert_eq!(err.http_status_code(), 502);
    }

    #[test]
    fn test_timeout_status() {
        let err = DomainError::Timeout;
        assert_eq!(err.http_status_code(), 408);
    }
}
