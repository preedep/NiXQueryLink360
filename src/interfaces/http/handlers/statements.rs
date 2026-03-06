use std::sync::Arc;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::error;

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

pub struct AppState {
    pub submit_uc: SubmitStatementUseCase,
    pub get_uc: GetStatementUseCase,
    pub cancel_uc: CancelStatementUseCase,
    pub default_warehouse_id: String,
}

fn domain_error_to_response(e: DomainError) -> Response {
    let status = StatusCode::from_u16(e.http_status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = json!({
        "error_code": format!("{:?}", e).split('{').next().unwrap_or("ERROR").trim().to_uppercase(),
        "message": e.to_string(),
    });
    (status, Json(body)).into_response()
}

/// POST /api/2.0/sql/statements
pub async fn submit_statement(
    Extension(state): Extension<Arc<AppState>>,
    Extension(BearerToken(token)): Extension<BearerToken>,
    Json(body): Json<StatementRequestDto>,
) -> Response {
    let wait_timeout_secs = body.parse_wait_timeout();
    let input = SubmitStatementInput {
        sql: body.statement,
        warehouse_id: body.warehouse_id.unwrap_or_default(),
        format: body.format,
        disposition: body.disposition,
        wait_timeout_secs,
        on_wait_timeout: body.on_wait_timeout,
        parameters: body.parameters.unwrap_or_default(),
        token,
    };

    match state.submit_uc.execute(input).await {
        Ok(result) => {
            let dto: StatementResponseDto = result.into();
            (StatusCode::OK, Json(dto)).into_response()
        }
        Err(e) => {
            error!(error = %e, "submit_statement failed");
            domain_error_to_response(e)
        }
    }
}

/// GET /api/2.0/sql/statements/:statement_id
pub async fn get_statement(
    Extension(state): Extension<Arc<AppState>>,
    Extension(BearerToken(token)): Extension<BearerToken>,
    Path(statement_id): Path<String>,
) -> Response {
    let input = GetStatementInput {
        statement_id,
        warehouse_id: state.default_warehouse_id.clone(),
        token,
    };

    match state.get_uc.execute(input).await {
        Ok(result) => {
            let dto: StatementResponseDto = result.into();
            (StatusCode::OK, Json(dto)).into_response()
        }
        Err(e) => {
            error!(error = %e, "get_statement failed");
            domain_error_to_response(e)
        }
    }
}

/// DELETE /api/2.0/sql/statements/:statement_id/cancel
pub async fn cancel_statement(
    Extension(state): Extension<Arc<AppState>>,
    Extension(BearerToken(token)): Extension<BearerToken>,
    Path(statement_id): Path<String>,
) -> Response {
    let input = CancelStatementInput {
        statement_id,
        warehouse_id: state.default_warehouse_id.clone(),
        token,
    };

    match state.cancel_uc.execute(input).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "cancelled"}))).into_response(),
        Err(e) => {
            error!(error = %e, "cancel_statement failed");
            domain_error_to_response(e)
        }
    }
}
