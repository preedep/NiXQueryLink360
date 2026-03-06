use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Execution format for results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementFormat {
    JsonArray,
    ArrowStream,
}

impl Default for StatementFormat {
    fn default() -> Self {
        StatementFormat::JsonArray
    }
}

/// Disposition: where results are returned
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementDisposition {
    Inline,
    ExternalLinks,
}

impl Default for StatementDisposition {
    fn default() -> Self {
        StatementDisposition::Inline
    }
}

/// What to do if wait_timeout expires before query completes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OnWaitTimeout {
    Continue,
    Cancel,
}

impl Default for OnWaitTimeout {
    fn default() -> Self {
        OnWaitTimeout::Continue
    }
}

/// State of a SQL statement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Closed,
}

/// Named parameter for parameterized queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementParameter {
    pub name: String,
    pub value: String,
    #[serde(rename = "type")]
    pub param_type: String,
}

/// Core domain entity: a SQL statement to be executed
#[derive(Debug, Clone)]
pub struct Statement {
    pub id: Uuid,
    pub sql: String,
    pub warehouse_id: String,
    pub format: StatementFormat,
    pub disposition: StatementDisposition,
    pub wait_timeout_secs: u64,
    pub on_wait_timeout: OnWaitTimeout,
    pub parameters: Vec<StatementParameter>,
    pub created_at: DateTime<Utc>,
}

impl Statement {
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

/// Result of a statement execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementResult {
    pub state: StatementState,
    pub statement_id: String,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub data: Option<serde_json::Value>,
    pub schema: Option<serde_json::Value>,
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
