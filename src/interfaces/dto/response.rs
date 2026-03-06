use serde::Serialize;
use crate::domain::entities::statement::{StatementResult, StatementState};

#[derive(Debug, Serialize)]
pub struct StatementResponseDto {
    pub statement_id: String,
    pub status: StatusDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultDto>,
}

#[derive(Debug, Serialize)]
pub struct StatusDto {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDto>,
}

#[derive(Debug, Serialize)]
pub struct ErrorDto {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResultDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_array: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_row_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponseDto {
    pub error_code: String,
    pub message: String,
}

impl From<StatementResult> for StatementResponseDto {
    fn from(r: StatementResult) -> Self {
        let state_str = match &r.state {
            StatementState::Pending => "PENDING",
            StatementState::Running => "RUNNING",
            StatementState::Succeeded => "SUCCEEDED",
            StatementState::Failed => "FAILED",
            StatementState::Cancelled => "CANCELLED",
            StatementState::Closed => "CLOSED",
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
            status: StatusDto {
                state: state_str.to_string(),
                error,
            },
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
