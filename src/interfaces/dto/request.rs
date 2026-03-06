use serde::Deserialize;
use crate::domain::entities::statement::{StatementFormat, StatementDisposition, OnWaitTimeout, StatementParameter};

/// Incoming HTTP request body for POST /api/2.0/sql/statements
#[derive(Debug, Deserialize)]
pub struct StatementRequestDto {
    pub statement: String,
    pub warehouse_id: Option<String>,
    pub wait_timeout: Option<String>,
    pub on_wait_timeout: Option<OnWaitTimeout>,
    pub format: Option<StatementFormat>,
    pub disposition: Option<StatementDisposition>,
    pub parameters: Option<Vec<StatementParameter>>,
}

impl StatementRequestDto {
    /// Parse wait_timeout string like "10s" → u64 seconds
    pub fn parse_wait_timeout(&self) -> Option<u64> {
        self.wait_timeout.as_ref().and_then(|t| {
            t.strip_suffix('s')
                .and_then(|n| n.parse::<u64>().ok())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dto(timeout: Option<&str>) -> StatementRequestDto {
        StatementRequestDto {
            statement: "SELECT 1".to_string(),
            warehouse_id: None,
            wait_timeout: timeout.map(str::to_string),
            on_wait_timeout: None,
            format: None,
            disposition: None,
            parameters: None,
        }
    }

    #[test]
    fn test_parse_wait_timeout_valid() {
        let dto = make_dto(Some("10s"));
        assert_eq!(dto.parse_wait_timeout(), Some(10));
    }

    #[test]
    fn test_parse_wait_timeout_zero() {
        let dto = make_dto(Some("0s"));
        assert_eq!(dto.parse_wait_timeout(), Some(0));
    }

    #[test]
    fn test_parse_wait_timeout_none() {
        let dto = make_dto(None);
        assert_eq!(dto.parse_wait_timeout(), None);
    }

    #[test]
    fn test_parse_wait_timeout_invalid_format() {
        let dto = make_dto(Some("10"));
        assert_eq!(dto.parse_wait_timeout(), None);
    }

    #[test]
    fn test_parse_wait_timeout_non_numeric() {
        let dto = make_dto(Some("tens"));
        assert_eq!(dto.parse_wait_timeout(), None);
    }
}
