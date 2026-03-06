use async_trait::async_trait;
use crate::domain::{
    entities::statement::{Statement, StatementResult},
    errors::DomainError,
};

/// Port: upstream SQL warehouse client
/// Infrastructure layer must implement this trait
#[async_trait]
pub trait WarehouseClient: Send + Sync {
    /// Submit a SQL statement and return the result (possibly async)
    async fn submit_statement(&self, statement: &Statement, token: &str) -> Result<StatementResult, DomainError>;

    /// Get the current result/status of a statement
    async fn get_statement(&self, statement_id: &str, warehouse_id: &str, token: &str) -> Result<StatementResult, DomainError>;

    /// Cancel a running statement
    async fn cancel_statement(&self, statement_id: &str, warehouse_id: &str, token: &str) -> Result<(), DomainError>;
}
