use std::sync::Arc;
use crate::domain::{
    entities::statement::StatementResult,
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};

pub struct GetStatementInput {
    pub statement_id: String,
    pub warehouse_id: String,
    pub token: String,
}

pub struct GetStatementUseCase {
    client: Arc<dyn WarehouseClient>,
}

impl GetStatementUseCase {
    pub fn new(client: Arc<dyn WarehouseClient>) -> Self {
        GetStatementUseCase { client }
    }

    pub async fn execute(&self, input: GetStatementInput) -> Result<StatementResult, DomainError> {
        if input.statement_id.trim().is_empty() {
            return Err(DomainError::InvalidRequest {
                message: "statement_id cannot be empty".to_string(),
            });
        }
        self.client
            .get_statement(&input.statement_id, &input.warehouse_id, &input.token)
            .await
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
