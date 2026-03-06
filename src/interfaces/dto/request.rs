//! Inbound Data Transfer Object for `POST /api/2.0/sql/statements`.
//!
//! [`StatementRequestDto`] is deserialized directly from the JSON request body
//! by Axum's `Json` extractor.  The handler then maps it to a use-case input
//! struct, applying defaults for any omitted optional fields.

use serde::Deserialize;
use crate::domain::entities::statement::{
    StatementFormat, StatementDisposition, OnWaitTimeout, StatementParameter,
};

/// JSON body accepted by `POST /api/2.0/sql/statements`.
///
/// All fields except `statement` are optional; sensible defaults are applied
/// by the submit handler when they are absent.
///
/// # Example (minimal)
/// ```json
/// { "statement": "SELECT 1" }
/// ```
///
/// # Example (full)
/// ```json
/// {
///   "statement": "SELECT * FROM orders WHERE id = :order_id",
///   "warehouse_id": "wh-prod",
///   "wait_timeout": "10s",
///   "on_wait_timeout": "CONTINUE",
///   "format": "JSON_ARRAY",
///   "disposition": "INLINE",
///   "parameters": [{ "name": "order_id", "value": "42", "type": "BIGINT" }]
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct StatementRequestDto {
    /// The SQL text to execute.  Required; must not be blank.
    pub statement: String,

    /// Target warehouse.  Falls back to `upstream.default_warehouse_id` if omitted.
    pub warehouse_id: Option<String>,

    /// Synchronous wait timeout in the format `"<n>s"`, e.g. `"10s"` or `"0s"`.
    /// Databricks maximum is `"50s"`.  `None` uses the default of `"0s"` (pure async).
    pub wait_timeout: Option<String>,

    /// Action when `wait_timeout` expires and the statement is still running.
    /// `None` defaults to [`OnWaitTimeout::Continue`].
    pub on_wait_timeout: Option<OnWaitTimeout>,

    /// Wire format for result data.  `None` defaults to [`StatementFormat::JsonArray`].
    pub format: Option<StatementFormat>,

    /// Where result data is delivered.  `None` defaults to [`StatementDisposition::Inline`].
    pub disposition: Option<StatementDisposition>,

    /// Named parameters for parameterized SQL.  `None` or empty means no parameters.
    pub parameters: Option<Vec<StatementParameter>>,
}

impl StatementRequestDto {
    /// Parse the `wait_timeout` string into an integer number of seconds.
    ///
    /// Accepts strings in the format `"<n>s"` (e.g. `"10s"`).  Returns `None`
    /// if the field is absent, the suffix is missing, or the numeric part is
    /// not a valid `u64`.
    ///
    /// # Examples
    /// - `"10s"` → `Some(10)`
    /// - `"0s"`  → `Some(0)`
    /// - `"10"`  → `None` (missing `s` suffix)
    /// - absent  → `None`
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
