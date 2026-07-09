use super::models::{ApiErrorBody, ApiResponse};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;

/// API key authentication middleware.
/// If `api_key` is None, all requests are allowed.
/// If set, checks `X-API-Key` header or `Authorization: Bearer <key>`.
pub async fn api_key_middleware(req: Request, next: Next, api_key: Option<String>) -> Response {
    let Some(expected_key) = api_key else {
        return next.run(req).await;
    };

    let provided = req
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    match provided {
        Some(key) if keys_match(&key, &expected_key) => next.run(req).await,
        _ => {
            tracing::warn!("unauthorized API request");
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(ApiResponse::<()>::err_body(ApiErrorBody::new(
                    "unauthorized",
                    "A valid API key is required.",
                ))),
            )
                .into_response()
        }
    }
}

fn keys_match(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::keys_match;

    #[test]
    fn api_keys_must_match_exactly() {
        assert!(keys_match("secret", "secret"));
        assert!(!keys_match("Secret", "secret"));
        assert!(!keys_match("secret ", "secret"));
        assert!(!keys_match("short", "a-longer-key"));
    }
}
