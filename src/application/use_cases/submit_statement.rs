//! Use case: submit a SQL statement to an upstream warehouse.

use std::sync::Arc;
use tracing::{debug, info, warn};
use crate::domain::{
    entities::statement::{
        Statement, StatementResult, StatementFormat, StatementDisposition,
        OnWaitTimeout, StatementParameter,
    },
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};

/// Input data transferred from the HTTP layer to this use case.
/// Contains everything needed to build and dispatch a [`Statement`].
pub struct SubmitStatementInput {
    /// Raw SQL text provided by the caller.
    pub sql: String,
    /// Target warehouse ID; falls back to the configured default when empty.
    pub warehouse_id: String,
    pub format: Option<StatementFormat>,
    pub disposition: Option<StatementDisposition>,
    /// Seconds to wait for a synchronous result (0–50). Defaults to 10.
    pub wait_timeout_secs: Option<u64>,
    pub on_wait_timeout: Option<OnWaitTimeout>,
    pub parameters: Vec<StatementParameter>,
    /// Bearer token forwarded to the upstream (may be the server-side env token).
    pub token: String,
}

/// Validates and dispatches a SQL statement to the upstream warehouse.
///
/// Responsibilities:
/// 1. Fall back to the default warehouse when none is specified.
/// 2. Construct an immutable [`Statement`] entity.
/// 3. Run domain validation before hitting the network.
/// 4. Delegate execution to the [`WarehouseClient`] port.
pub struct SubmitStatementUseCase {
    client: Arc<dyn WarehouseClient>,
    /// Warehouse used when the caller omits `warehouse_id`.
    default_warehouse_id: String,
}

impl SubmitStatementUseCase {
    /// Create a new use case wired to the given client and default warehouse.
    pub fn new(client: Arc<dyn WarehouseClient>, default_warehouse_id: String) -> Self {
        SubmitStatementUseCase { client, default_warehouse_id }
    }

    /// Execute the use case.
    ///
    /// # Errors
    /// - [`DomainError::InvalidRequest`] — validation failed (empty SQL, bad timeout, …)
    /// - [`DomainError::WarehouseNotFound`] — no config for the resolved warehouse ID
    /// - [`DomainError::UpstreamError`] — the Databricks API returned an error
    /// - [`DomainError::AuthenticationFailed`] — the token was rejected
    pub async fn execute(&self, input: SubmitStatementInput) -> Result<StatementResult, DomainError> {
        // Resolve warehouse: use caller-supplied value or fall back to default
        let warehouse_id = if input.warehouse_id.is_empty() {
            debug!(
                default_warehouse = %self.default_warehouse_id,
                "No warehouse_id in request; using default"
            );
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

        // Validate before touching the network — fail fast
        statement.validate().map_err(|msg| {
            warn!(statement_id = %statement.id, reason = %msg, "Statement validation failed");
            DomainError::InvalidRequest { message: msg }
        })?;

        info!(
            statement_id = %statement.id,
            warehouse_id = %statement.warehouse_id,
            wait_timeout_secs = statement.wait_timeout_secs,
            "Submitting statement to upstream"
        );

        let start = std::time::Instant::now();
        let result = self.client.submit_statement(&statement, &input.token).await;
        let elapsed_ms = start.elapsed().as_millis();

        match &result {
            Ok(r) => info!(
                statement_id = %statement.id,
                upstream_statement_id = %r.statement_id,
                state = ?r.state,
                elapsed_ms,
                "Statement submitted successfully"
            ),
            Err(e) => warn!(
                statement_id = %statement.id,
                error = %e,
                elapsed_ms,
                "Statement submission failed"
            ),
        }

        result
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
