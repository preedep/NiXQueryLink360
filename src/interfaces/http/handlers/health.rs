//! Health and readiness probe handlers.
//!
//! These endpoints are intentionally unauthenticated so that load balancers,
//! Kubernetes probes, and container orchestrators can check the service
//! without needing credentials.
//!
//! | Endpoint       | Purpose                                        |
//! |----------------|------------------------------------------------|
//! | `GET /health`  | Liveness — confirms the process is running     |
//! | `GET /ready`   | Readiness — confirms the server is accepting requests |

use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

/// `GET /health` — Liveness probe.
///
/// Returns `200 OK` as long as the server process is alive.
/// The response body includes the service name and the binary's version
/// so operators can quickly confirm which build is running.
///
/// # Response
/// ```json
/// { "status": "ok", "service": "NiXQueryLink360", "version": "0.1.0" }
/// ```
pub async fn health_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "service": "NiXQueryLink360",
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
}

/// `GET /ready` — Readiness probe.
///
/// Returns `200 OK` when the server is ready to accept SQL statement requests.
/// In Phase 1 this is equivalent to the liveness check; a future phase may
/// gate readiness on upstream warehouse connectivity.
///
/// # Response
/// ```json
/// { "status": "ready" }
/// ```
pub async fn readiness_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
        })),
    )
}
