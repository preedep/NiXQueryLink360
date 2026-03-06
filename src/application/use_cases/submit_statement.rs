use std::sync::Arc;
use crate::domain::{
    entities::statement::{
        Statement, StatementResult, StatementFormat, StatementDisposition,
        OnWaitTimeout, StatementParameter,
    },
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};

pub struct SubmitStatementInput {
    pub sql: String,
    pub warehouse_id: String,
    pub format: Option<StatementFormat>,
    pub disposition: Option<StatementDisposition>,
    pub wait_timeout_secs: Option<u64>,
    pub on_wait_timeout: Option<OnWaitTimeout>,
    pub parameters: Vec<StatementParameter>,
    pub token: String,
}

pub struct SubmitStatementUseCase {
    client: Arc<dyn WarehouseClient>,
    default_warehouse_id: String,
}

impl SubmitStatementUseCase {
    pub fn new(client: Arc<dyn WarehouseClient>, default_warehouse_id: String) -> Self {
        SubmitStatementUseCase {
            client,
            default_warehouse_id,
        }
    }

    pub async fn execute(&self, input: SubmitStatementInput) -> Result<StatementResult, DomainError> {
        let warehouse_id = if input.warehouse_id.is_empty() {
            self.default_warehouse_id.clone()
        } else {
            input.warehouse_id
        };

        let statement = Statement::new(
            input.sql,
            warehouse_id,
            input.format.unwrap_or_default(),
            input.disposition.unwrap_or_default(),
            input.wait_timeout_secs.unwrap_or(10),
            input.on_wait_timeout.unwrap_or_default(),
            input.parameters,
        );

        statement.validate().map_err(|msg| DomainError::InvalidRequest { message: msg })?;

        self.client.submit_statement(&statement, &input.token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::domain::entities::statement::StatementState;

    struct MockWarehouseClient {
        should_fail: bool,
    }

    #[async_trait]
    impl WarehouseClient for MockWarehouseClient {
        async fn submit_statement(&self, statement: &Statement, _token: &str) -> Result<StatementResult, DomainError> {
            if self.should_fail {
                return Err(DomainError::UpstreamError { message: "mock error".to_string() });
            }
            Ok(StatementResult {
                state: StatementState::Succeeded,
                statement_id: statement.id.to_string(),
                error_message: None,
                error_code: None,
                data: Some(serde_json::json!([["row1"]])),
                schema: None,
                total_row_count: Some(1),
            })
        }

        async fn get_statement(&self, statement_id: &str, _warehouse_id: &str, _token: &str) -> Result<StatementResult, DomainError> {
            Ok(StatementResult {
                state: StatementState::Succeeded,
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

    fn make_input(sql: &str, warehouse_id: &str) -> SubmitStatementInput {
        SubmitStatementInput {
            sql: sql.to_string(),
            warehouse_id: warehouse_id.to_string(),
            format: None,
            disposition: None,
            wait_timeout_secs: None,
            on_wait_timeout: None,
            parameters: vec![],
            token: "test-token".to_string(),
        }
    }

    #[tokio::test]
    async fn test_submit_valid_statement_succeeds() {
        let client = Arc::new(MockWarehouseClient { should_fail: false });
        let uc = SubmitStatementUseCase::new(client, "default-wh".to_string());
        let result = uc.execute(make_input("SELECT 1", "wh1")).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().state, StatementState::Succeeded);
    }

    #[tokio::test]
    async fn test_submit_empty_sql_returns_invalid_request() {
        let client = Arc::new(MockWarehouseClient { should_fail: false });
        let uc = SubmitStatementUseCase::new(client, "default-wh".to_string());
        let result = uc.execute(make_input("", "wh1")).await;
        assert!(matches!(result, Err(DomainError::InvalidRequest { .. })));
    }

    #[tokio::test]
    async fn test_submit_uses_default_warehouse_when_empty() {
        let client = Arc::new(MockWarehouseClient { should_fail: false });
        let uc = SubmitStatementUseCase::new(client, "default-wh".to_string());
        // Empty warehouse_id should use default
        let result = uc.execute(make_input("SELECT 1", "")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_submit_upstream_error_propagated() {
        let client = Arc::new(MockWarehouseClient { should_fail: true });
        let uc = SubmitStatementUseCase::new(client, "default-wh".to_string());
        let result = uc.execute(make_input("SELECT 1", "wh1")).await;
        assert!(matches!(result, Err(DomainError::UpstreamError { .. })));
    }
}
