//! Use case: poll the current status and result of a submitted statement.

use std::sync::Arc;
use tracing::{debug, info, warn};
use crate::domain::{
    entities::statement::StatementResult,
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};

/// Input for polling a previously submitted statement.
pub struct GetStatementInput {
    /// Databricks statement ID returned by the submit call.
    pub statement_id: String,
    /// Warehouse the statement belongs to (used for routing).
    pub warehouse_id: String,
    pub token: String,
}

/// Fetches the current state and result of an existing statement.
///
/// This use case is called repeatedly by clients that use the
/// async (poll) pattern: submit → poll until `SUCCEEDED | FAILED | CANCELLED`.
pub struct GetStatementUseCase {
    client: Arc<dyn WarehouseClient>,
}

impl GetStatementUseCase {
    pub fn new(client: Arc<dyn WarehouseClient>) -> Self {
        GetStatementUseCase { client }
    }

    /// Execute the use case.
    ///
    /// # Errors
    /// - [`DomainError::InvalidRequest`] — `statement_id` is empty
    /// - [`DomainError::StatementNotFound`] — ID not found in the upstream
    /// - [`DomainError::UpstreamError`] — upstream returned an unexpected error
    pub async fn execute(&self, input: GetStatementInput) -> Result<StatementResult, DomainError> {
        if input.statement_id.trim().is_empty() {
            warn!("get_statement called with empty statement_id");
            return Err(DomainError::InvalidRequest {
                message: "statement_id cannot be empty".to_string(),
            });
        }

        debug!(statement_id = %input.statement_id, "Polling statement status");

        let start = std::time::Instant::now();
        let result = self.client
            .get_statement(&input.statement_id, &input.warehouse_id, &input.token)
            .await;
        let elapsed_ms = start.elapsed().as_millis();

        match &result {
            Ok(r) => info!(
                statement_id = %input.statement_id,
                state = ?r.state,
                elapsed_ms,
                "Statement poll succeeded"
            ),
            Err(e) => warn!(
                statement_id = %input.statement_id,
                error = %e,
                elapsed_ms,
                "Statement poll failed"
            ),
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::domain::entities::statement::{Statement, StatementState};

    struct MockClient;

    #[async_trait]
    impl WarehouseClient for MockClient {
        async fn submit_statement(&self, statement: &Statement, _token: &str) -> Result<StatementResult, DomainError> {
            Ok(StatementResult {
                state: StatementState::Succeeded,
                statement_id: statement.id.to_string(),
                error_message: None,
                error_code: None,
                data: None,
                schema: None,
                total_row_count: None,
            })
        }

        async fn get_statement(&self, statement_id: &str, _warehouse_id: &str, _token: &str) -> Result<StatementResult, DomainError> {
            if statement_id == "not-found" {
                return Err(DomainError::StatementNotFound {
                    statement_id: statement_id.to_string(),
                });
            }
            Ok(StatementResult {
                state: StatementState::Running,
                statement_id: statement_id.to_string(),
                error_message: None,
                error_code: None,
                data: None,
                schema: None,
                total_row_count: None,
            })
        }

        async fn cancel_statement(&self, _statement_id: &str, _warehouse_id: &str, _token: &str) -> Result<(), DomainError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_get_existing_statement() {
        let uc = GetStatementUseCase::new(Arc::new(MockClient));
        let result = uc.execute(GetStatementInput {
            statement_id: "stmt-123".to_string(),
            warehouse_id: "wh1".to_string(),
            token: "token".to_string(),
        }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().state, StatementState::Running);
    }

    #[tokio::test]
    async fn test_get_not_found_statement() {
        let uc = GetStatementUseCase::new(Arc::new(MockClient));
        let result = uc.execute(GetStatementInput {
            statement_id: "not-found".to_string(),
            warehouse_id: "wh1".to_string(),
            token: "token".to_string(),
        }).await;
        assert!(matches!(result, Err(DomainError::StatementNotFound { .. })));
    }

    #[tokio::test]
    async fn test_get_empty_statement_id_fails() {
        let uc = GetStatementUseCase::new(Arc::new(MockClient));
        let result = uc.execute(GetStatementInput {
            statement_id: "".to_string(),
            warehouse_id: "wh1".to_string(),
            token: "token".to_string(),
        }).await;
        assert!(matches!(result, Err(DomainError::InvalidRequest { .. })));
    }
}
