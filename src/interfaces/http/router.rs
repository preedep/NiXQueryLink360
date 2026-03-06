//! Axum router factory — wires routes, middleware layers, and shared state.
//!
//! [`create_router`] is called once at startup.  It performs dependency
//! injection in the following order:
//! 1. Build infrastructure ([`DatabricksClient`] + [`RetryPolicy`])
//! 2. Wire application use cases (`submit`, `get`, `cancel`)
//! 3. Register routes in two groups (authenticated API + public probes)
//! 4. Apply global middleware layers (request ID, tracing, CORS)
//!
//! # Middleware stack (outermost → innermost)
//! | Layer                    | Responsibility                                       |
//! |--------------------------|------------------------------------------------------|
//! | `SetRequestIdLayer`      | Generate a UUID for each request (`x-request-id`)   |
//! | `PropagateRequestIdLayer`| Echo the request ID back in the response             |
//! | `CorsLayer::permissive`  | Allow cross-origin requests (Phase 1 — tighten later)|
//! | `TraceLayer`             | Emit `tower-http` span logs for every request        |
//! | `Extension(state)`       | Inject shared `AppState` into handlers               |
//! | `require_auth` (per-route)| Enforce Bearer token on SQL API routes only         |

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

/// Build and return the fully configured Axum [`Router`].
///
/// This function constructs the entire application object graph:
/// infrastructure adapters → use cases → handlers → middleware layers.
///
/// # Errors
/// Returns an [`anyhow::Error`] if the `reqwest` HTTP client cannot be built
/// (e.g. TLS initialisation failure).
pub fn create_router(settings: Settings) -> Result<Router> {
    // ── Infrastructure ────────────────────────────────────────────────────────
    let retry = RetryPolicy::new(settings.retry.max_attempts, settings.retry.base_delay_ms);

    let db_client = Arc::new(DatabricksClient::new(
        settings.upstream.warehouses.clone(),
        settings.pool.max_connections,
        settings.pool.connection_timeout_secs,
        retry,
    )?);

    let default_wh = settings.upstream.default_warehouse_id.clone();

    // ── Application (use cases) ───────────────────────────────────────────────
    let state = Arc::new(AppState {
        submit_uc: SubmitStatementUseCase::new(db_client.clone(), default_wh.clone()),
        get_uc:    GetStatementUseCase::new(db_client.clone()),
        cancel_uc: CancelStatementUseCase::new(db_client),
        default_warehouse_id: default_wh,
    });

    // ── Routes ────────────────────────────────────────────────────────────────

    // SQL API endpoints — all require a valid Bearer token.
    let api_routes = Router::new()
        .route("/api/2.0/sql/statements",                          post(submit_statement))
        .route("/api/2.0/sql/statements/{statement_id}",           get(get_statement))
        .route("/api/2.0/sql/statements/{statement_id}/cancel",    delete(cancel_statement))
        .layer(middleware::from_fn(require_auth));

    // Probe endpoints — unauthenticated for load-balancer / k8s health checks.
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route("/ready",  get(readiness_check));

    // ── Middleware stack ──────────────────────────────────────────────────────
    // Layers are applied outermost-first; SetRequestIdLayer runs before any
    // handler so every log line carries a correlation ID.
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
