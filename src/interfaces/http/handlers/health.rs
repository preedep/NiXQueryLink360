use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

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

pub async fn readiness_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
        })),
    )
}
