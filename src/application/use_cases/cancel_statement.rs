use std::sync::Arc;
use crate::domain::{
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};

pub struct CancelStatementInput {
    pub statement_id: String,
    pub warehouse_id: String,
    pub token: String,
}

pub struct CancelStatementUseCase {
    client: Arc<dyn WarehouseClient>,
}

impl CancelStatementUseCase {
    pub fn new(client: Arc<dyn WarehouseClient>) -> Self {
        CancelStatementUseCase { client }
    }

    pub async fn execute(&self, input: CancelStatementInput) -> Result<(), DomainError> {
        if input.statement_id.trim().is_empty() {
            return Err(DomainError::InvalidRequest {
                message: "statement_id cannot be empty".to_string(),
            });
        }
        self.client
            .cancel_statement(&input.statement_id, &input.warehouse_id, &input.token)
            .await
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
