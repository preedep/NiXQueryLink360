use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Extract Bearer token from Authorization header
pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Axum middleware: require Bearer token, inject into request extensions
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

/// Newtype wrapper to carry the token through axum extensions
#[derive(Clone)]
pub struct BearerToken(pub String);

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
