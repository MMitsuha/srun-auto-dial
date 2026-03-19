use super::models::*;
use crate::error::SrunError;
use crate::service::{SrunService, parse_mac};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

type AppState = Arc<SrunService>;

/// Convert SrunError into an axum HTTP response.
fn error_response(e: SrunError) -> Response {
    let status = e.status_code();
    let body = Json(ApiResponse::<()>::err(e.to_string()));
    (status, body).into_response()
}

pub async fn health() -> impl IntoResponse {
    Json(ApiResponse::ok("ok"))
}

pub async fn status(
    State(service): State<AppState>,
    Query(q): Query<StatusQuery>,
) -> Response {
    match service.get_status(&q.interface).await {
        Ok(s) => (StatusCode::OK, Json(ApiResponse::ok(s))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn list_interfaces(State(service): State<AppState>) -> Response {
    match service.list_interfaces().await {
        Ok(links) => {
            let infos: Vec<InterfaceInfo> = links
                .into_iter()
                .map(|l| InterfaceInfo {
                    index: l.index,
                    name: l.name,
                })
                .collect();
            (StatusCode::OK, Json(ApiResponse::ok(infos))).into_response()
        }
        Err(e) => error_response(e),
    }
}

pub async fn login_local(
    State(service): State<AppState>,
    Json(req): Json<LocalLoginRequest>,
) -> Response {
    match service
        .login_local(&req.interface, &req.username, &req.password)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::ok(result))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn logout_local(
    State(service): State<AppState>,
    Json(req): Json<LocalLogoutRequest>,
) -> Response {
    match service.logout_local(&req.interface).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::<()>::ok_empty())).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn login_macvlan(
    State(service): State<AppState>,
    Json(req): Json<MacvlanLoginRequest>,
) -> Response {
    let mac = match parse_mac(&req.mac_address) {
        Ok(m) => m,
        Err(e) => return error_response(e),
    };

    match service
        .login_macvlan(&req.parent_interface, &mac, &req.username, &req.password)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::ok(result))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn logout_macvlan(
    State(service): State<AppState>,
    Json(req): Json<MacvlanLogoutRequest>,
) -> Response {
    match service.logout_macvlan(&req.parent_interface).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::<()>::ok_empty())).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn login_random(
    State(service): State<AppState>,
    Json(req): Json<RandomLoginRequest>,
) -> Response {
    if req.count == 0 || req.count > 100 {
        return error_response(SrunError::Config(
            "count must be between 1 and 100".to_string(),
        ));
    }

    match service
        .login_random(&req.parent_interface, req.count)
        .await
    {
        Ok(results) => (StatusCode::OK, Json(ApiResponse::ok(results))).into_response(),
        Err(e) => error_response(e),
    }
}
