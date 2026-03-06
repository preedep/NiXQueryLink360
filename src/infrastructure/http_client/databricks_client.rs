use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error};

use crate::domain::{
    entities::statement::{
        Statement, StatementResult, StatementState, StatementFormat, StatementDisposition,
    },
    errors::DomainError,
    ports::warehouse_client::WarehouseClient,
};
use crate::infrastructure::config::settings::WarehouseSettings;
use super::retry::RetryPolicy;

// ---------- Databricks API DTOs (all owned — required for retry cloning) ----------

/// Request body sent to Databricks Statement Execution API v2.0
#[derive(Debug, Serialize, Clone)]
struct DatabricksStatementRequest {
    statement: String,
    warehouse_id: String,
    wait_timeout: String,
    on_wait_timeout: String,
    format: String,
    disposition: String,
}

#[derive(Debug, Deserialize)]
struct DatabricksStatementResponse {
    statement_id: String,
    status: DatabricksStatus,
    manifest: Option<serde_json::Value>,
    result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct DatabricksStatus {
    state: String,
    error: Option<DatabricksError>,
}

#[derive(Debug, Deserialize)]
struct DatabricksError {
    message: String,
    error_code: Option<String>,
}

// ---------- Client ----------

pub struct DatabricksClient {
    http: Client,
    warehouses: HashMap<String, WarehouseSettings>,
    retry: RetryPolicy,
}

impl DatabricksClient {
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

    fn get_warehouse(&self, warehouse_id: &str) -> Result<&WarehouseSettings, DomainError> {
        self.warehouses
            .get(warehouse_id)
            .ok_or_else(|| DomainError::WarehouseNotFound {
                warehouse_id: warehouse_id.to_string(),
            })
    }

    /// Resolve token: prefer env var over fallback
    fn resolve_token(&self, warehouse: &WarehouseSettings, fallback_token: &str) -> String {
        std::env::var(&warehouse.token_env)
            .unwrap_or_else(|_| fallback_token.to_string())
    }

    fn map_state(state_str: &str) -> StatementState {
        match state_str {
            "PENDING"   => StatementState::Pending,
            "RUNNING"   => StatementState::Running,
            "SUCCEEDED" => StatementState::Succeeded,
            "FAILED"    => StatementState::Failed,
            "CANCELLED" => StatementState::Cancelled,
            "CLOSED"    => StatementState::Closed,
            _           => StatementState::Failed,
        }
    }

    fn format_str(format: &StatementFormat) -> &'static str {
        match format {
            StatementFormat::JsonArray   => "JSON_ARRAY",
            StatementFormat::ArrowStream => "ARROW_STREAM",
        }
    }

    fn disposition_str(disposition: &StatementDisposition) -> &'static str {
        match disposition {
            StatementDisposition::Inline        => "INLINE",
            StatementDisposition::ExternalLinks => "EXTERNAL_LINKS",
        }
    }

    /// Convert raw API response into domain StatementResult
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

#[async_trait]
impl WarehouseClient for DatabricksClient {
    async fn submit_statement(
        &self,
        statement: &Statement,
        token: &str,
    ) -> Result<StatementResult, DomainError> {
        let warehouse = self.get_warehouse(&statement.warehouse_id)?;
        let resolved_token = self.resolve_token(warehouse, token);
        let url = format!("https://{}/api/2.0/sql/statements", warehouse.host);

        // Build owned body (Clone-able) so the retry closure can re-use it
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

        debug!(url = %url, warehouse_id = %statement.warehouse_id, "Submitting statement");

        // Clone all values the retry closure needs — reqwest::Client is Arc-backed (cheap clone)
        let http    = self.http.clone();
        let tok     = resolved_token.clone();
        let req_url = url.clone();
        let req_body = body.clone();

        let resp = self.retry.execute(|| {
            let c  = http.clone();
            let t  = tok.clone();
            let u  = req_url.clone();
            let b  = req_body.clone();
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
                message: "Databricks rejected the token".to_string(),
            });
        }
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            error!(status = status, body = %err_text, "Upstream error on submit");
            return Err(DomainError::UpstreamError {
                message: format!("HTTP {}: {}", status, err_text),
            });
        }

        let db_resp: DatabricksStatementResponse = resp.json().await.map_err(|e| {
            DomainError::UpstreamError {
                message: format!("Failed to parse submit response: {}", e),
            }
        })?;

        Ok(Self::parse_response(db_resp))
    }

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

        debug!(url = %url, statement_id = %statement_id, "Getting statement");

        let resp = self.http
            .get(&url)
            .bearer_auth(&resolved_token)
            .send()
            .await
            .map_err(|e| DomainError::UpstreamError { message: e.to_string() })?;

        if resp.status().as_u16() == 404 {
            return Err(DomainError::StatementNotFound {
                statement_id: statement_id.to_string(),
            });
        }
        if !resp.status().is_success() {
            let status_code = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(DomainError::UpstreamError {
                message: format!("HTTP {}: {}", status_code, err_text),
            });
        }

        let db_resp: DatabricksStatementResponse = resp.json().await.map_err(|e| {
            DomainError::UpstreamError {
                message: format!("Failed to parse get response: {}", e),
            }
        })?;

        Ok(Self::parse_response(db_resp))
    }

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

        debug!(url = %url, statement_id = %statement_id, "Cancelling statement");

        let resp = self.http
            .post(&url)
            .bearer_auth(&resolved_token)
            .send()
            .await
            .map_err(|e| DomainError::UpstreamError { message: e.to_string() })?;

        if !resp.status().is_success() {
            let status_code = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(DomainError::UpstreamError {
                message: format!("HTTP {}: {}", status_code, err_text),
            });
        }

        Ok(())
    }
}

// ---------- Unit Tests ----------

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
