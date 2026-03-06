use std::sync::Arc;
use anyhow::Result;
use axum::{
    middleware,
    routing::{delete, get, post},
    Extension, Router,
};
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::{
    application::use_cases::{
        cancel_statement::CancelStatementUseCase,
        get_statement::GetStatementUseCase,
        submit_statement::SubmitStatementUseCase,
    },
    infrastructure::{
        config::settings::Settings,
        http_client::{
            databricks_client::DatabricksClient,
            retry::RetryPolicy,
        },
    },
    interfaces::http::{
        handlers::{
            health::{health_check, readiness_check},
            statements::{cancel_statement, get_statement, submit_statement, AppState},
        },
        middleware::auth::require_auth,
    },
};

pub fn create_router(settings: Settings) -> Result<Router> {
    // Build infrastructure
    let retry = RetryPolicy::new(settings.retry.max_attempts, settings.retry.base_delay_ms);

    let db_client = Arc::new(DatabricksClient::new(
        settings.upstream.warehouses.clone(),
        settings.pool.max_connections,
        settings.pool.connection_timeout_secs,
        retry,
    )?);

    let default_wh = settings.upstream.default_warehouse_id.clone();

    // Wire use cases
    let state = Arc::new(AppState {
        submit_uc: SubmitStatementUseCase::new(db_client.clone(), default_wh.clone()),
        get_uc: GetStatementUseCase::new(db_client.clone()),
        cancel_uc: CancelStatementUseCase::new(db_client),
        default_warehouse_id: default_wh,
    });

    // SQL API routes (require auth)
    let api_routes = Router::new()
        .route("/api/2.0/sql/statements", post(submit_statement))
        .route("/api/2.0/sql/statements/{statement_id}", get(get_statement))
        .route("/api/2.0/sql/statements/{statement_id}/cancel", delete(cancel_statement))
        .layer(middleware::from_fn(require_auth));

    // Public routes
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check));

    let router = Router::new()
        .merge(api_routes)
        .merge(public_routes)
        .layer(Extension(state))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    Ok(router)
}
