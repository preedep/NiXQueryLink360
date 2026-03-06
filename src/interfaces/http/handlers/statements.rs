//! Axum HTTP handlers for the Databricks Statement Execution API v2.0 endpoints.
//!
//! Each handler follows the same pattern:
//! 1. Extract auth token and path/body from the request.
//! 2. Build a use-case input struct.
//! 3. Delegate to the appropriate use case.
//! 4. Map the result to an HTTP response.
//!
//! Handlers are deliberately thin — all business logic lives in the
//! `application` layer. Handlers only translate between HTTP and domain types.

use std::sync::Arc;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{error, info};

use crate::{
    application::use_cases::{
        cancel_statement::{CancelStatementInput, CancelStatementUseCase},
        get_statement::{GetStatementInput, GetStatementUseCase},
        submit_statement::{SubmitStatementInput, SubmitStatementUseCase},
    },
    domain::errors::DomainError,
    interfaces::{
        dto::{request::StatementRequestDto, response::StatementResponseDto},
        http::middleware::auth::BearerToken,
    },
};

/// Shared application state injected into every handler via [`axum::Extension`].
///
/// Wired once at startup in [`crate::interfaces::http::router::create_router`]
/// and shared across all concurrent requests via [`Arc`].
pub struct AppState {
    pub submit_uc: SubmitStatementUseCase,
    pub get_uc: GetStatementUseCase,
    pub cancel_uc: CancelStatementUseCase,
    /// Default warehouse used when the request omits `warehouse_id`.
    pub default_warehouse_id: String,
}

/// Convert a [`DomainError`] into an Axum [`Response`] with the appropriate
/// HTTP status code and a JSON error body compatible with the Databricks API.
fn domain_error_to_response(e: DomainError) -> Response {
    let status = StatusCode::from_u16(e.http_status_code())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    // Derive a short SCREAMING_SNAKE_CASE error code from the variant name
    let error_code = format!("{:?}", e)
        .split('{')
        .next()
        .unwrap_or("ERROR")
        .trim()
        .to_uppercase();
    let body = json!({ "error_code": error_code, "message": e.to_string() });
    (status, Json(body)).into_response()
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// `POST /api/2.0/sql/statements`
///
/// Submit a SQL statement for execution. Returns either a synchronous result
/// (when `wait_timeout` is set and the query finishes in time) or a statement
/// ID that the client must poll with [`get_statement`].
pub async fn submit_statement(
    Extension(state): Extension<Arc<AppState>>,
    Extension(BearerToken(token)): Extension<BearerToken>,
    Json(body): Json<StatementRequestDto>,
) -> Response {
    let warehouse_id = body.warehouse_id.clone().unwrap_or_default();
    let wait_timeout_secs = body.parse_wait_timeout();

    info!(
        warehouse_id = %warehouse_id,
        wait_timeout_secs = ?wait_timeout_secs,
        "POST /api/2.0/sql/statements"
    );

    let input = SubmitStatementInput {
        sql: body.statement,
        warehouse_id,
        format: body.format,
        disposition: body.disposition,
        wait_timeout_secs,
        on_wait_timeout: body.on_wait_timeout,
        parameters: body.parameters.unwrap_or_default(),
        token,
    };

    let start = std::time::Instant::now();
    match state.submit_uc.execute(input).await {
        Ok(result) => {
            info!(
                upstream_statement_id = %result.statement_id,
                state = ?result.state,
                elapsed_ms = start.elapsed().as_millis(),
                "submit_statement → OK"
            );
            let dto: StatementResponseDto = result.into();
            (StatusCode::OK, Json(dto)).into_response()
        }
        Err(e) => {
            error!(
                error = %e,
                elapsed_ms = start.elapsed().as_millis(),
                "submit_statement → error"
            );
            domain_error_to_response(e)
        }
    }
}

/// `GET /api/2.0/sql/statements/{statement_id}`
///
/// Poll the current status and result of a previously submitted statement.
/// Clients should call this endpoint until `state` is `SUCCEEDED`, `FAILED`,
/// or `CANCELLED`.
pub async fn get_statement(
    Extension(state): Extension<Arc<AppState>>,
    Extension(BearerToken(token)): Extension<BearerToken>,
    Path(statement_id): Path<String>,
) -> Response {
    info!(statement_id = %statement_id, "GET /api/2.0/sql/statements/{{id}}");

    let input = GetStatementInput {
        statement_id: statement_id.clone(),
        warehouse_id: state.default_warehouse_id.clone(),
        token,
    };

    let start = std::time::Instant::now();
    match state.get_uc.execute(input).await {
        Ok(result) => {
            info!(
                statement_id = %statement_id,
                state = ?result.state,
                elapsed_ms = start.elapsed().as_millis(),
                "get_statement → OK"
            );
            let dto: StatementResponseDto = result.into();
            (StatusCode::OK, Json(dto)).into_response()
        }
        Err(e) => {
            error!(
                statement_id = %statement_id,
                error = %e,
                elapsed_ms = start.elapsed().as_millis(),
                "get_statement → error"
            );
            domain_error_to_response(e)
        }
    }
}

/// `DELETE /api/2.0/sql/statements/{statement_id}/cancel`
///
/// Request cancellation of a statement that is `PENDING` or `RUNNING`.
/// Cancellation is best-effort; if the statement has already completed
/// the upstream will return an error.
pub async fn cancel_statement(
    Extension(state): Extension<Arc<AppState>>,
    Extension(BearerToken(token)): Extension<BearerToken>,
    Path(statement_id): Path<String>,
) -> Response {
    info!(statement_id = %statement_id, "DELETE /api/2.0/sql/statements/{{id}}/cancel");

    let input = CancelStatementInput {
        statement_id: statement_id.clone(),
        warehouse_id: state.default_warehouse_id.clone(),
        token,
    };

    let start = std::time::Instant::now();
    match state.cancel_uc.execute(input).await {
        Ok(_) => {
            info!(
                statement_id = %statement_id,
                elapsed_ms = start.elapsed().as_millis(),
                "cancel_statement → OK"
            );
            (StatusCode::OK, Json(json!({"status": "cancelled"}))).into_response()
        }
        Err(e) => {
            error!(
                statement_id = %statement_id,
                error = %e,
                elapsed_ms = start.elapsed().as_millis(),
                "cancel_statement → error"
            );
            domain_error_to_response(e)
        }
    }
}
