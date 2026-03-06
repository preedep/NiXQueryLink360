//! Core SQL statement entities.
//!
//! These types represent the *language* of the domain — what a SQL statement is,
//! what states it can be in, and what a result looks like — without any knowledge
//! of HTTP, databases, or external frameworks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wire format for query results returned by the upstream warehouse.
///
/// `JsonArray` (default) returns rows as a 2-D JSON array and is suitable
/// for most REST clients. `ArrowStream` returns columnar binary data and is
/// reserved for Phase 2 (high-throughput analytics clients).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementFormat {
    /// Rows encoded as `[["col1_val", "col2_val"], ...]`. Default.
    JsonArray,
    /// Binary Apache Arrow IPC stream. Phase 2 only.
    ArrowStream,
}

impl Default for StatementFormat {
    fn default() -> Self {
        StatementFormat::JsonArray
    }
}

/// Where the result data is delivered after the statement completes.
///
/// `Inline` (default) embeds the result directly in the API response body.
/// `ExternalLinks` returns pre-signed URLs to download large result sets —
/// reserved for Phase 2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementDisposition {
    /// Result data is included in the response body. Default.
    Inline,
    /// Result data is available via pre-signed download URLs.
    ExternalLinks,
}

impl Default for StatementDisposition {
    fn default() -> Self {
        StatementDisposition::Inline
    }
}

/// Action to take when `wait_timeout` expires before the statement finishes.
///
/// `Continue` (default) returns the statement ID so the caller can poll.
/// `Cancel` aborts execution if not done within the timeout window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OnWaitTimeout {
    /// Return immediately with status RUNNING; caller must poll. Default.
    Continue,
    /// Cancel the statement if it has not completed by the timeout.
    Cancel,
}

impl Default for OnWaitTimeout {
    fn default() -> Self {
        OnWaitTimeout::Continue
    }
}

/// Lifecycle state of a SQL statement in the upstream warehouse.
///
/// Transitions: `Pending → Running → Succeeded | Failed | Cancelled`.
/// `Closed` means the result set was consumed and is no longer available.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementState {
    /// Accepted but not yet dispatched to a worker.
    Pending,
    /// Currently executing on the warehouse cluster.
    Running,
    /// Completed successfully; results are available.
    Succeeded,
    /// Execution failed; `error_message` and `error_code` are populated.
    Failed,
    /// Cancelled by the client before or during execution.
    Cancelled,
    /// Result set has been fully read and is no longer stored.
    Closed,
}

/// A typed, named parameter for parameterized SQL queries.
///
/// Parameterized queries prevent SQL injection when user-supplied values
/// are embedded in statements.
///
/// # Example (JSON)
/// ```json
/// { "name": "user_id", "value": "42", "type": "BIGINT" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementParameter {
    /// Parameter name as referenced in the SQL (e.g. `:user_id`).
    pub name: String,
    /// Value as a string; the warehouse coerces it to `param_type`.
    pub value: String,
    /// Databricks SQL type string, e.g. `"STRING"`, `"BIGINT"`, `"TIMESTAMP"`.
    #[serde(rename = "type")]
    pub param_type: String,
}

/// Core domain entity representing a SQL statement to be executed.
///
/// A `Statement` is immutable once created. Validation is performed eagerly
/// via [`Statement::validate`] before the statement is forwarded upstream.
#[derive(Debug, Clone)]
pub struct Statement {
    /// Unique identifier generated at creation time (UUIDv4).
    pub id: Uuid,
    /// The raw SQL text to execute.
    pub sql: String,
    /// Target warehouse; routed by the infrastructure layer.
    pub warehouse_id: String,
    /// Result serialization format requested by the caller.
    pub format: StatementFormat,
    /// Controls where result data is delivered.
    pub disposition: StatementDisposition,
    /// Maximum seconds to block waiting for a result (0–50).
    pub wait_timeout_secs: u64,
    /// What to do if the statement is still running after `wait_timeout_secs`.
    pub on_wait_timeout: OnWaitTimeout,
    /// Optional named parameters for parameterized queries.
    pub parameters: Vec<StatementParameter>,
    /// Wall-clock time this statement was received by the proxy (UTC).
    pub created_at: DateTime<Utc>,
}

impl Statement {
    /// Create a new `Statement` with a freshly generated UUID and current timestamp.
    pub fn new(
        sql: String,
        warehouse_id: String,
        format: StatementFormat,
        disposition: StatementDisposition,
        wait_timeout_secs: u64,
        on_wait_timeout: OnWaitTimeout,
        parameters: Vec<StatementParameter>,
    ) -> Self {
        Statement {
            id: Uuid::new_v4(),
            sql,
            warehouse_id,
            format,
            disposition,
            wait_timeout_secs,
            on_wait_timeout,
            parameters,
            created_at: Utc::now(),
        }
    }

    /// Validate domain invariants before forwarding to the upstream.
    ///
    /// # Errors
    /// Returns a human-readable `String` describing the first violation:
    /// - SQL text is blank or whitespace-only
    /// - `warehouse_id` is blank
    /// - `wait_timeout_secs` exceeds the Databricks maximum of 50 s
    pub fn validate(&self) -> Result<(), String> {
        if self.sql.trim().is_empty() {
            return Err("SQL statement cannot be empty".to_string());
        }
        if self.warehouse_id.trim().is_empty() {
            return Err("warehouse_id cannot be empty".to_string());
        }
        if self.wait_timeout_secs > 50 {
            return Err("wait_timeout cannot exceed 50 seconds".to_string());
        }
        Ok(())
    }
}

/// Result returned after submitting or polling a statement.
///
/// This is the *domain representation* of a Databricks statement response.
/// The interface layer converts it into [`StatementResponseDto`] for the wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementResult {
    /// Current lifecycle state of the statement.
    pub state: StatementState,
    /// Databricks-assigned statement identifier used for polling.
    pub statement_id: String,
    /// Human-readable error description when `state == Failed`.
    pub error_message: Option<String>,
    /// Machine-readable error code when `state == Failed` (e.g. `"SYNTAX_ERROR"`).
    pub error_code: Option<String>,
    /// Rows as a 2-D JSON array when `format == JsonArray` and `state == Succeeded`.
    pub data: Option<serde_json::Value>,
    /// Column schema metadata from the result manifest.
    pub schema: Option<serde_json::Value>,
    /// Total rows in the complete result set (may differ from rows in this chunk).
    pub total_row_count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_statement(sql: &str, warehouse_id: &str, timeout: u64) -> Statement {
        Statement::new(
            sql.to_string(),
            warehouse_id.to_string(),
            StatementFormat::default(),
            StatementDisposition::default(),
            timeout,
            OnWaitTimeout::default(),
            vec![],
        )
    }

    #[test]
    fn test_statement_new_generates_unique_ids() {
        let s1 = make_statement("SELECT 1", "wh1", 10);
        let s2 = make_statement("SELECT 1", "wh1", 10);
        assert_ne!(s1.id, s2.id);
    }

    #[test]
    fn test_validate_empty_sql_fails() {
        let s = make_statement("   ", "wh1", 10);
        assert!(s.validate().is_err());
        assert!(s.validate().unwrap_err().contains("empty"));
    }

    #[test]
    fn test_validate_empty_warehouse_id_fails() {
        let s = make_statement("SELECT 1", "", 10);
        assert!(s.validate().is_err());
        assert!(s.validate().unwrap_err().contains("warehouse_id"));
    }

    #[test]
    fn test_validate_timeout_exceeds_limit_fails() {
        let s = make_statement("SELECT 1", "wh1", 51);
        assert!(s.validate().is_err());
        assert!(s.validate().unwrap_err().contains("50 seconds"));
    }

    #[test]
    fn test_validate_valid_statement_passes() {
        let s = make_statement("SELECT 1", "wh1", 10);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn test_validate_max_timeout_passes() {
        let s = make_statement("SELECT 1", "wh1", 50);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn test_default_format_is_json_array() {
        assert_eq!(StatementFormat::default(), StatementFormat::JsonArray);
    }

    #[test]
    fn test_default_disposition_is_inline() {
        assert_eq!(StatementDisposition::default(), StatementDisposition::Inline);
    }

    #[test]
    fn test_state_serialization() {
        let state = StatementState::Succeeded;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"SUCCEEDED\"");
    }
}
