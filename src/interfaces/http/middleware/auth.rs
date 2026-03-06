//! Bearer token authentication middleware for Axum.
//!
//! All SQL API routes are protected by [`require_auth`].  The middleware
//! extracts the token from the `Authorization` header and injects it as a
//! [`BearerToken`] extension so downstream handlers can forward it to the
//! upstream Databricks API.
//!
//! The proxy does **not** validate the token itself — validation is delegated
//! to Databricks.  This keeps the proxy stateless and avoids needing its own
//! token store.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Newtype wrapper that carries the Bearer token through Axum request extensions.
///
/// Handlers extract it via `Extension<BearerToken>` and forward it to the
/// upstream Databricks API.
#[derive(Clone)]
pub struct BearerToken(pub String);

/// Extract a Bearer token from an HTTP `Authorization` header.
///
/// Returns `None` if:
/// - The `Authorization` header is absent
/// - The scheme is not `Bearer`
/// - The token value is empty or whitespace-only after trimming
///
/// The returned token has leading and trailing whitespace stripped.
pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Axum middleware that enforces Bearer token authentication.
///
/// If a valid token is present it is injected into the request extensions
/// as [`BearerToken`] and the request is passed to the next handler.
///
/// If the token is missing or malformed the middleware short-circuits with
/// `401 Unauthorized` and a JSON error body compatible with the Databricks
/// API error format.
///
/// # Response on failure
/// ```json
/// {
///   "error_code": "UNAUTHENTICATED",
///   "message": "Missing or invalid Authorization header. Use: Bearer <token>"
/// }
/// ```
pub async fn require_auth(request: Request, next: Next) -> Response {
    let token = extract_bearer_token(request.headers());

    match token {
        Some(t) => {
            let mut req = request;
            req.extensions_mut().insert(BearerToken(t));
            next.run(req).await
        }
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error_code": "UNAUTHENTICATED",
                "message": "Missing or invalid Authorization header. Use: Bearer <token>"
            })),
        ).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_valid_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer my-secret-token"));
        assert_eq!(extract_bearer_token(&headers), Some("my-secret-token".to_string()));
    }

    #[test]
    fn test_extract_missing_auth_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn test_extract_non_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Basic dXNlcjpwYXNz"));
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn test_extract_bearer_with_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer   token-with-space  "));
        assert_eq!(extract_bearer_token(&headers), Some("token-with-space".to_string()));
    }

    #[test]
    fn test_extract_empty_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer "));
        assert_eq!(extract_bearer_token(&headers), None);
    }
}
