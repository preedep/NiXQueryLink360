//! Outbound Data Transfer Objects for statement endpoint responses.
//!
//! [`StatementResponseDto`] is the JSON body returned by both the submit
//! (`POST`) and poll (`GET`) endpoints and mirrors the structure of the
//! Databricks Statement Execution API v2.0 response schema.
//!
//! Optional fields are omitted from serialization when `None`, keeping the
//! response compact and spec-compliant.

use serde::Serialize;
use crate::domain::entities::statement::{StatementResult, StatementState};

/// Top-level response body for statement endpoints.
///
/// # Example (running)
/// ```json
/// { "statement_id": "01ef…", "status": { "state": "RUNNING" } }
/// ```
///
/// # Example (succeeded with inline data)
/// ```json
/// {
///   "statement_id": "01ef…",
///   "status": { "state": "SUCCEEDED" },
///   "result": { "data_array": [["1"]], "total_row_count": 1 }
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct StatementResponseDto {
    /// Databricks-assigned statement identifier (UUID-like string).
    pub statement_id: String,

    /// Current lifecycle state and any error details.
    pub status: StatusDto,

    /// Present only when `state == SUCCEEDED` and result data is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultDto>,
}

/// Lifecycle state and optional error information.
#[derive(Debug, Serialize)]
pub struct StatusDto {
    /// SCREAMING_SNAKE_CASE state string, e.g. `"RUNNING"`, `"SUCCEEDED"`.
    pub state: String,

    /// Populated only when `state == "FAILED"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDto>,
}

/// Error details returned when a statement fails.
#[derive(Debug, Serialize)]
pub struct ErrorDto {
    /// Human-readable description of the failure.
    pub message: String,

    /// Machine-readable error code, e.g. `"PARSE_SYNTAX_ERROR"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// Inline query result data.
///
/// Present in the response body only when `disposition == INLINE` and
/// the statement has `SUCCEEDED`.
#[derive(Debug, Serialize)]
pub struct ResultDto {
    /// Rows encoded as a 2-D JSON array: `[["col1_val", "col2_val"], …]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_array: Option<serde_json::Value>,

    /// Total number of rows in the complete result set.
    /// May be larger than the rows returned in this response if pagination is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_row_count: Option<i64>,
}

/// Generic error response body for non-statement error paths (4xx/5xx).
///
/// Compatible with the Databricks API error format so clients can handle
/// proxy errors the same way as upstream errors.
#[derive(Debug, Serialize)]
pub struct ErrorResponseDto {
    /// SCREAMING_SNAKE_CASE error code, e.g. `"INVALID_REQUEST"`.
    pub error_code: String,
    /// Human-readable description of the error.
    pub message: String,
}

// ── Conversion ────────────────────────────────────────────────────────────────

impl From<StatementResult> for StatementResponseDto {
    /// Convert a domain [`StatementResult`] into the wire-format DTO.
    ///
    /// The `result` field is populated only when `data` or `total_row_count`
    /// is present (i.e. the statement has `SUCCEEDED` with inline data).
    fn from(r: StatementResult) -> Self {
        let state_str = match &r.state {
            StatementState::Pending   => "PENDING",
            StatementState::Running   => "RUNNING",
            StatementState::Succeeded => "SUCCEEDED",
            StatementState::Failed    => "FAILED",
            StatementState::Cancelled => "CANCELLED",
            StatementState::Closed    => "CLOSED",
        };

        let error = r.error_message.map(|msg| ErrorDto {
            message: msg,
            error_code: r.error_code,
        });

        let result = if r.data.is_some() || r.total_row_count.is_some() {
            Some(ResultDto {
                data_array: r.data,
                total_row_count: r.total_row_count,
            })
        } else {
            None
        };

        StatementResponseDto {
            statement_id: r.statement_id,
            status: StatusDto { state: state_str.to_string(), error },
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(state: StatementState) -> StatementResult {
        StatementResult {
            state,
            statement_id: "stmt-1".to_string(),
            error_message: None,
            error_code: None,
            data: None,
            schema: None,
            total_row_count: None,
        }
    }

    #[test]
    fn test_succeeded_state_serialization() {
        let dto: StatementResponseDto = make_result(StatementState::Succeeded).into();
        assert_eq!(dto.status.state, "SUCCEEDED");
        assert!(dto.status.error.is_none());
        assert!(dto.result.is_none());
    }

    #[test]
    fn test_failed_state_includes_error() {
        let result = StatementResult {
            state: StatementState::Failed,
            statement_id: "stmt-2".to_string(),
            error_message: Some("query failed".to_string()),
            error_code: Some("SYNTAX_ERROR".to_string()),
            data: None,
            schema: None,
            total_row_count: None,
        };
        let dto: StatementResponseDto = result.into();
        assert_eq!(dto.status.state, "FAILED");
        assert!(dto.status.error.is_some());
        let error = dto.status.error.unwrap();
        assert_eq!(error.message, "query failed");
        assert_eq!(error.error_code, Some("SYNTAX_ERROR".to_string()));
    }

    #[test]
    fn test_result_with_data_included() {
        let result = StatementResult {
            state: StatementState::Succeeded,
            statement_id: "stmt-3".to_string(),
            error_message: None,
            error_code: None,
            data: Some(serde_json::json!([["a", "b"]])),
            schema: None,
            total_row_count: Some(1),
        };
        let dto: StatementResponseDto = result.into();
        assert!(dto.result.is_some());
        let res = dto.result.unwrap();
        assert!(res.data_array.is_some());
        assert_eq!(res.total_row_count, Some(1));
    }

    #[test]
    fn test_statement_id_preserved() {
        let dto: StatementResponseDto = make_result(StatementState::Running).into();
        assert_eq!(dto.statement_id, "stmt-1");
        assert_eq!(dto.status.state, "RUNNING");
    }
}
