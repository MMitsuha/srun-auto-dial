use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// API key authentication middleware.
/// If `api_key` is None, all requests are allowed.
/// If set, checks `X-API-Key` header or `Authorization: Bearer <key>`.
pub async fn api_key_middleware(
    req: Request,
    next: Next,
    api_key: Option<String>,
) -> Response {
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
        Some(key) if key == expected_key => next.run(req).await,
        _ => {
            tracing::warn!("unauthorized API request");
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({
                    "success": false,
                    "error": "unauthorized: missing or invalid API key"
                })),
            )
                .into_response()
        }
    }
}
