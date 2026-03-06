use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Statement not found: {statement_id}")]
    StatementNotFound { statement_id: String },

    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Upstream error: {message}")]
    UpstreamError { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Warehouse not found: {warehouse_id}")]
    WarehouseNotFound { warehouse_id: String },

    #[error("Statement cancelled")]
    StatementCancelled,

    #[error("Request timeout")]
    Timeout,
}

impl DomainError {
    pub fn http_status_code(&self) -> u16 {
        match self {
            DomainError::StatementNotFound { .. } => 404,
            DomainError::InvalidRequest { .. } => 400,
            DomainError::AuthenticationFailed { .. } => 401,
            DomainError::WarehouseNotFound { .. } => 404,
            DomainError::UpstreamError { .. } => 502,
            DomainError::StatementCancelled => 200,
            DomainError::Timeout => 408,
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
