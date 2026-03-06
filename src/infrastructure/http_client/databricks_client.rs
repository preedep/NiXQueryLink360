//! Concrete implementation of [`WarehouseClient`] backed by the
//! Databricks Statement Execution REST API v2.0.
//!
//! [`DatabricksClient`] uses a shared `reqwest::Client` connection pool and
//! delegates retry logic to [`RetryPolicy`].  All internal DTOs use owned
//! `String` fields so they can be cheaply cloned into retry closures.
//!
//! # Token resolution
//! The Bearer token is resolved at request time in the following order:
//! 1. Environment variable named by `warehouse.token_env` (highest priority)
//! 2. Token forwarded by the caller via the `Authorization` header
//!
//! This design allows per-warehouse service accounts while still permitting
//! caller-supplied tokens for fine-grained access control.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::domain::{
    entities::statement::{
        Statement, StatementResult, StatementState, StatementFormat, StatementDisposition,
    },
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};
use crate::infrastructure::config::settings::WarehouseSettings;
use super::retry::RetryPolicy;

// ── Databricks API DTOs ───────────────────────────────────────────────────────
// All fields are owned Strings so the structs implement Clone and can be
// captured by the retry closure without lifetime issues.

/// JSON request body for `POST /api/2.0/sql/statements`.
#[derive(Debug, Serialize, Clone)]
struct DatabricksStatementRequest {
    /// The SQL text to execute.
    statement: String,
    /// Target warehouse identifier.
    warehouse_id: String,
    /// Synchronous wait timeout, e.g. `"10s"`. `"0s"` means pure async.
    wait_timeout: String,
    /// Action on timeout: `"CONTINUE"` or `"CANCEL"`.
    on_wait_timeout: String,
    /// Result format: `"JSON_ARRAY"` or `"ARROW_STREAM"`.
    format: String,
    /// Result delivery: `"INLINE"` or `"EXTERNAL_LINKS"`.
    disposition: String,
}

/// JSON response body for statement submit and poll endpoints.
#[derive(Debug, Deserialize)]
struct DatabricksStatementResponse {
    statement_id: String,
    status: DatabricksStatus,
    /// Column schema and pagination metadata (present when `SUCCEEDED`).
    manifest: Option<serde_json::Value>,
    /// Result rows (present when `SUCCEEDED` and disposition is `INLINE`).
    result: Option<serde_json::Value>,
}

/// Lifecycle state and optional error from the Databricks API.
#[derive(Debug, Deserialize)]
struct DatabricksStatus {
    /// State string, e.g. `"RUNNING"`, `"SUCCEEDED"`, `"FAILED"`.
    state: String,
    /// Populated when `state == "FAILED"`.
    error: Option<DatabricksError>,
}

/// Error details returned by Databricks when a statement fails.
#[derive(Debug, Deserialize)]
struct DatabricksError {
    /// Human-readable description of the failure.
    message: String,
    /// Machine-readable error code, e.g. `"PARSE_SYNTAX_ERROR"`.
    error_code: Option<String>,
}

// ── Client ────────────────────────────────────────────────────────────────────

/// `reqwest`-backed implementation of [`WarehouseClient`].
///
/// One instance is created at startup and shared across all concurrent
/// requests via [`std::sync::Arc`].  The underlying `reqwest::Client` is
/// `Arc`-backed internally, so cloning it is cheap.
pub struct DatabricksClient {
    /// Shared connection pool — `Arc`-backed; cheap to clone.
    http: Client,
    /// Map of warehouse ID → connection settings, populated at startup.
    warehouses: HashMap<String, WarehouseSettings>,
    /// Retry policy applied to submit requests.
    retry: RetryPolicy,
}

impl DatabricksClient {
    /// Construct a new client from configuration.
    ///
    /// # Parameters
    /// - `warehouses`            — list of warehouse configurations
    /// - `pool_max_connections`  — max idle connections per host
    /// - `connect_timeout_secs`  — TCP connect timeout
    /// - `retry`                 — retry policy for submit requests
    ///
    /// # Errors
    /// Returns an [`anyhow::Error`] if the underlying `reqwest` client
    /// cannot be built (e.g. invalid TLS configuration).
    pub fn new(
        warehouses: Vec<WarehouseSettings>,
        pool_max_connections: u32,
        connect_timeout_secs: u64,
        retry: RetryPolicy,
    ) -> Result<Self, anyhow::Error> {
        let http = Client::builder()
            .pool_max_idle_per_host(pool_max_connections as usize)
            .connect_timeout(Duration::from_secs(connect_timeout_secs))
            .timeout(Duration::from_secs(120))
            .https_only(true)
            .build()?;

        let warehouses = warehouses
            .into_iter()
            .map(|w| (w.id.clone(), w))
            .collect();

        Ok(DatabricksClient { http, warehouses, retry })
    }

    /// Look up warehouse settings by ID.
    ///
    /// # Errors
    /// Returns [`DomainError::WarehouseNotFound`] if `warehouse_id` is not
    /// present in the configured warehouse map.
    fn get_warehouse(&self, warehouse_id: &str) -> Result<&WarehouseSettings, DomainError> {
        self.warehouses
            .get(warehouse_id)
            .ok_or_else(|| DomainError::WarehouseNotFound {
                warehouse_id: warehouse_id.to_string(),
            })
    }

    /// Resolve the Bearer token to use for a request.
    ///
    /// If the environment variable named by `warehouse.token_env` is set
    /// and non-empty, it takes precedence over the caller-supplied token.
    /// This allows per-warehouse service-account tokens to be injected at
    /// runtime without changing the API surface.
    fn resolve_token(&self, warehouse: &WarehouseSettings, fallback_token: &str) -> String {
        std::env::var(&warehouse.token_env)
            .unwrap_or_else(|_| fallback_token.to_string())
    }

    /// Map a Databricks state string to the domain [`StatementState`] enum.
    ///
    /// Unknown strings (e.g. future Databricks additions) are mapped to
    /// `Failed` as a safe fallback so callers always receive a terminal state.
    fn map_state(state_str: &str) -> StatementState {
        match state_str {
            "PENDING"   => StatementState::Pending,
            "RUNNING"   => StatementState::Running,
            "SUCCEEDED" => StatementState::Succeeded,
            "FAILED"    => StatementState::Failed,
            "CANCELLED" => StatementState::Cancelled,
            "CLOSED"    => StatementState::Closed,
            other => {
                debug!(state = other, "Unknown Databricks state — falling back to Failed");
                StatementState::Failed
            }
        }
    }

    /// Convert a [`StatementFormat`] variant to the Databricks API string.
    fn format_str(format: &StatementFormat) -> &'static str {
        match format {
            StatementFormat::JsonArray   => "JSON_ARRAY",
            StatementFormat::ArrowStream => "ARROW_STREAM",
        }
    }

    /// Convert a [`StatementDisposition`] variant to the Databricks API string.
    fn disposition_str(disposition: &StatementDisposition) -> &'static str {
        match disposition {
            StatementDisposition::Inline        => "INLINE",
            StatementDisposition::ExternalLinks => "EXTERNAL_LINKS",
        }
    }

    /// Convert a raw Databricks API response into the domain [`StatementResult`].
    ///
    /// Extracts `data_array` from `result` and `schema` / `total_row_count`
    /// from `manifest` when present.
    fn parse_response(db_resp: DatabricksStatementResponse) -> StatementResult {
        StatementResult {
            state: Self::map_state(&db_resp.status.state),
            statement_id: db_resp.statement_id,
            error_message: db_resp.status.error.as_ref().map(|e| e.message.clone()),
            error_code: db_resp.status.error.as_ref().and_then(|e| e.error_code.clone()),
            data: db_resp.result
                .as_ref()
                .and_then(|r| r.get("data_array").cloned()),
            schema: db_resp.manifest
                .as_ref()
                .and_then(|m| m.get("schema").cloned()),
            total_row_count: db_resp.manifest
                .as_ref()
                .and_then(|m| m.get("total_row_count"))
                .and_then(|v| v.as_i64()),
        }
    }
}

// ── WarehouseClient implementation ───────────────────────────────────────────

#[async_trait]
impl WarehouseClient for DatabricksClient {
    /// Submit a SQL statement and return the initial (or final) result.
    ///
    /// Retries the POST request according to the configured [`RetryPolicy`]
    /// on transient status codes (`429`, `5xx`).
    async fn submit_statement(
        &self,
        statement: &Statement,
        token: &str,
    ) -> Result<StatementResult, DomainError> {
        let warehouse = self.get_warehouse(&statement.warehouse_id)?;
        let resolved_token = self.resolve_token(warehouse, token);
        let url = format!("https://{}/api/2.0/sql/statements", warehouse.host);

        // Build an owned, Clone-able body so the retry closure can re-use it
        // without lifetime issues.
        let body = DatabricksStatementRequest {
            statement:      statement.sql.clone(),
            warehouse_id:   statement.warehouse_id.clone(),
            wait_timeout:   format!("{}s", statement.wait_timeout_secs),
            on_wait_timeout: match &statement.on_wait_timeout {
                crate::domain::entities::statement::OnWaitTimeout::Continue => "CONTINUE".to_string(),
                crate::domain::entities::statement::OnWaitTimeout::Cancel   => "CANCEL".to_string(),
            },
            format:      Self::format_str(&statement.format).to_string(),
            disposition: Self::disposition_str(&statement.disposition).to_string(),
        };

        debug!(
            url = %url,
            warehouse_id = %statement.warehouse_id,
            statement_id = %statement.id,
            "POSTing statement to Databricks"
        );

        // reqwest::Client is Arc-backed — clone is cheap.
        let http     = self.http.clone();
        let tok      = resolved_token.clone();
        let req_url  = url.clone();
        let req_body = body.clone();

        let resp = self.retry.execute(|| {
            let c = http.clone();
            let t = tok.clone();
            let u = req_url.clone();
            let b = req_body.clone();
            async move {
                c.post(&u)
                    .bearer_auth(&t)
                    .json(&b)
                    .send()
                    .await
                    .map_err(|e| DomainError::UpstreamError { message: e.to_string() })
            }
        }).await?;

        let status = resp.status().as_u16();

        if status == 401 || status == 403 {
            return Err(DomainError::AuthenticationFailed {
                message: "Databricks rejected the Bearer token".to_string(),
            });
        }
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            error!(
                status,
                body = %err_text,
                warehouse_id = %statement.warehouse_id,
                "Databricks returned error on submit"
            );
            return Err(DomainError::UpstreamError {
                message: format!("HTTP {}: {}", status, err_text),
            });
        }

        let db_resp: DatabricksStatementResponse = resp.json().await.map_err(|e| {
            DomainError::UpstreamError {
                message: format!("Failed to parse submit response: {}", e),
            }
        })?;

        info!(
            statement_id = %db_resp.statement_id,
            state = %db_resp.status.state,
            "Databricks accepted statement"
        );

        Ok(Self::parse_response(db_resp))
    }

    /// Poll the current state and result of an existing statement.
    async fn get_statement(
        &self,
        statement_id: &str,
        warehouse_id: &str,
        token: &str,
    ) -> Result<StatementResult, DomainError> {
        let warehouse = self.get_warehouse(warehouse_id)?;
        let resolved_token = self.resolve_token(warehouse, token);
        let url = format!(
            "https://{}/api/2.0/sql/statements/{}",
            warehouse.host, statement_id
        );

        debug!(
            url = %url,
            statement_id = %statement_id,
            "GETting statement from Databricks"
        );

        let resp = self.http
            .get(&url)
            .bearer_auth(&resolved_token)
            .send()
            .await
            .map_err(|e| DomainError::UpstreamError { message: e.to_string() })?;

        let status = resp.status().as_u16();

        if status == 404 {
            return Err(DomainError::StatementNotFound {
                statement_id: statement_id.to_string(),
            });
        }
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            error!(
                status,
                body = %err_text,
                statement_id = %statement_id,
                "Databricks returned error on get"
            );
            return Err(DomainError::UpstreamError {
                message: format!("HTTP {}: {}", status, err_text),
            });
        }

        let db_resp: DatabricksStatementResponse = resp.json().await.map_err(|e| {
            DomainError::UpstreamError {
                message: format!("Failed to parse get response: {}", e),
            }
        })?;

        Ok(Self::parse_response(db_resp))
    }

    /// Send a cancellation request for an in-flight statement.
    async fn cancel_statement(
        &self,
        statement_id: &str,
        warehouse_id: &str,
        token: &str,
    ) -> Result<(), DomainError> {
        let warehouse = self.get_warehouse(warehouse_id)?;
        let resolved_token = self.resolve_token(warehouse, token);
        let url = format!(
            "https://{}/api/2.0/sql/statements/{}/cancel",
            warehouse.host, statement_id
        );

        debug!(
            url = %url,
            statement_id = %statement_id,
            "POSTing cancel request to Databricks"
        );

        let resp = self.http
            .post(&url)
            .bearer_auth(&resolved_token)
            .send()
            .await
            .map_err(|e| DomainError::UpstreamError { message: e.to_string() })?;

        let status = resp.status().as_u16();

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            error!(
                status,
                body = %err_text,
                statement_id = %statement_id,
                "Databricks returned error on cancel"
            );
            return Err(DomainError::UpstreamError {
                message: format!("HTTP {}: {}", status, err_text),
            });
        }

        info!(statement_id = %statement_id, "Databricks accepted cancellation request");
        Ok(())
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_state_succeeded() {
        assert_eq!(DatabricksClient::map_state("SUCCEEDED"), StatementState::Succeeded);
    }

    #[test]
    fn test_map_state_failed() {
        assert_eq!(DatabricksClient::map_state("FAILED"), StatementState::Failed);
    }

    #[test]
    fn test_map_state_pending() {
        assert_eq!(DatabricksClient::map_state("PENDING"), StatementState::Pending);
    }

    #[test]
    fn test_map_state_running() {
        assert_eq!(DatabricksClient::map_state("RUNNING"), StatementState::Running);
    }

    #[test]
    fn test_map_state_cancelled() {
        assert_eq!(DatabricksClient::map_state("CANCELLED"), StatementState::Cancelled);
    }

    #[test]
    fn test_map_state_closed() {
        assert_eq!(DatabricksClient::map_state("CLOSED"), StatementState::Closed);
    }

    #[test]
    fn test_map_state_unknown_falls_back_to_failed() {
        assert_eq!(DatabricksClient::map_state("BOGUS"), StatementState::Failed);
    }

    #[test]
    fn test_format_str_json_array() {
        assert_eq!(DatabricksClient::format_str(&StatementFormat::JsonArray), "JSON_ARRAY");
    }

    #[test]
    fn test_format_str_arrow() {
        assert_eq!(DatabricksClient::format_str(&StatementFormat::ArrowStream), "ARROW_STREAM");
    }

    #[test]
    fn test_disposition_str_inline() {
        assert_eq!(DatabricksClient::disposition_str(&StatementDisposition::Inline), "INLINE");
    }

    #[test]
    fn test_disposition_str_external() {
        assert_eq!(
            DatabricksClient::disposition_str(&StatementDisposition::ExternalLinks),
            "EXTERNAL_LINKS"
        );
    }

    #[test]
    fn test_warehouse_not_found() {
        let client = DatabricksClient {
            http: Client::new(),
            warehouses: HashMap::new(),
            retry: RetryPolicy::new(3, 100),
        };
        assert!(matches!(
            client.get_warehouse("nonexistent"),
            Err(DomainError::WarehouseNotFound { .. })
        ));
    }

    #[test]
    fn test_parse_response_succeeded() {
        let resp = DatabricksStatementResponse {
            statement_id: "stmt-1".to_string(),
            status: DatabricksStatus {
                state: "SUCCEEDED".to_string(),
                error: None,
            },
            manifest: Some(serde_json::json!({ "total_row_count": 5 })),
            result: Some(serde_json::json!({ "data_array": [["a"]] })),
        };
        let result = DatabricksClient::parse_response(resp);
        assert_eq!(result.state, StatementState::Succeeded);
        assert_eq!(result.total_row_count, Some(5));
        assert!(result.data.is_some());
    }

    #[test]
    fn test_parse_response_failed_includes_error() {
        let resp = DatabricksStatementResponse {
            statement_id: "stmt-2".to_string(),
            status: DatabricksStatus {
                state: "FAILED".to_string(),
                error: Some(DatabricksError {
                    message: "syntax error".to_string(),
                    error_code: Some("PARSE_SYNTAX_ERROR".to_string()),
                }),
            },
            manifest: None,
            result: None,
        };
        let result = DatabricksClient::parse_response(resp);
        assert_eq!(result.state, StatementState::Failed);
        assert_eq!(result.error_message, Some("syntax error".to_string()));
        assert_eq!(result.error_code, Some("PARSE_SYNTAX_ERROR".to_string()));
    }

    #[test]
    fn test_request_body_is_cloneable() {
        let body = DatabricksStatementRequest {
            statement:       "SELECT 1".to_string(),
            warehouse_id:    "wh-1".to_string(),
            wait_timeout:    "10s".to_string(),
            on_wait_timeout: "CONTINUE".to_string(),
            format:          "JSON_ARRAY".to_string(),
            disposition:     "INLINE".to_string(),
        };
        let cloned = body.clone();
        assert_eq!(cloned.statement, "SELECT 1");
    }
}
