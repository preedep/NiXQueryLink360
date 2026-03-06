//! Use case: cancel a running SQL statement.

use std::sync::Arc;
use tracing::{info, warn};
use crate::domain::{
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};

/// Input for cancelling a statement that is PENDING or RUNNING.
pub struct CancelStatementInput {
    /// Databricks statement ID to cancel.
    pub statement_id: String,
    /// Warehouse the statement belongs to (used for routing).
    pub warehouse_id: String,
    pub token: String,
}

/// Requests cancellation of an in-flight SQL statement.
///
/// Cancellation is best-effort — if the statement has already reached a
/// terminal state (`SUCCEEDED`, `FAILED`, `CANCELLED`) the upstream may
/// return an error, which is propagated as [`DomainError::UpstreamError`].
pub struct CancelStatementUseCase {
    client: Arc<dyn WarehouseClient>,
}

impl CancelStatementUseCase {
    pub fn new(client: Arc<dyn WarehouseClient>) -> Self {
        CancelStatementUseCase { client }
    }

    /// Execute the use case.
    ///
    /// # Errors
    /// - [`DomainError::InvalidRequest`] — `statement_id` is empty
    /// - [`DomainError::UpstreamError`] — upstream returned an unexpected error
    pub async fn execute(&self, input: CancelStatementInput) -> Result<(), DomainError> {
        if input.statement_id.trim().is_empty() {
            warn!("cancel_statement called with empty statement_id");
            return Err(DomainError::InvalidRequest {
                message: "statement_id cannot be empty".to_string(),
            });
        }

        info!(statement_id = %input.statement_id, "Requesting statement cancellation");

        let result = self.client
            .cancel_statement(&input.statement_id, &input.warehouse_id, &input.token)
            .await;

        match &result {
            Ok(_)  => info!(statement_id = %input.statement_id, "Statement cancelled successfully"),
            Err(e) => warn!(statement_id = %input.statement_id, error = %e, "Statement cancellation failed"),
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::domain::entities::statement::{Statement, StatementResult, StatementState};

    struct MockClient { should_fail: bool }

    #[async_trait]
    impl WarehouseClient for MockClient {
        async fn submit_statement(&self, s: &Statement, _: &str) -> Result<StatementResult, DomainError> {
            Ok(StatementResult { state: StatementState::Succeeded, statement_id: s.id.to_string(), error_message: None, error_code: None, data: None, schema: None, total_row_count: None })
        }
        async fn get_statement(&self, id: &str, _: &str, _: &str) -> Result<StatementResult, DomainError> {
            Ok(StatementResult { state: StatementState::Succeeded, statement_id: id.to_string(), error_message: None, error_code: None, data: None, schema: None, total_row_count: None })
        }
        async fn cancel_statement(&self, _: &str, _: &str, _: &str) -> Result<(), DomainError> {
            if self.should_fail {
                return Err(DomainError::UpstreamError { message: "cancel failed".to_string() });
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_cancel_valid_statement() {
        let uc = CancelStatementUseCase::new(Arc::new(MockClient { should_fail: false }));
        let result = uc.execute(CancelStatementInput {
            statement_id: "stmt-123".to_string(),
            warehouse_id: "wh1".to_string(),
            token: "token".to_string(),
        }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cancel_empty_id_fails() {
        let uc = CancelStatementUseCase::new(Arc::new(MockClient { should_fail: false }));
        let result = uc.execute(CancelStatementInput {
            statement_id: "".to_string(),
            warehouse_id: "wh1".to_string(),
            token: "token".to_string(),
        }).await;
        assert!(matches!(result, Err(DomainError::InvalidRequest { .. })));
    }

    #[tokio::test]
    async fn test_cancel_upstream_error_propagated() {
        let uc = CancelStatementUseCase::new(Arc::new(MockClient { should_fail: true }));
        let result = uc.execute(CancelStatementInput {
            statement_id: "stmt-123".to_string(),
            warehouse_id: "wh1".to_string(),
            token: "token".to_string(),
        }).await;
        assert!(matches!(result, Err(DomainError::UpstreamError { .. })));
    }
}
