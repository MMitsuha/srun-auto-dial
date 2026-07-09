use super::models::*;
use crate::error::SrunError;
use crate::service::{SrunService, parse_mac};
use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use tracing::{error, warn};

type AppState = Arc<SrunService>;

/// Convert SrunError into an axum HTTP response.
fn error_response(e: SrunError) -> Response {
    let status = e.status_code();
    if e.is_server_error() {
        error!(code = e.code(), error = %e, "API request failed");
    } else {
        warn!(code = e.code(), error = %e, "API request rejected");
    }
    let body = Json(ApiResponse::<()>::err(&e));
    (status, body).into_response()
}

fn rejection_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(ApiResponse::<()>::err_body(ApiErrorBody::new(
            code, message,
        ))),
    )
        .into_response()
}

fn parse_json<T>(payload: std::result::Result<Json<T>, JsonRejection>) -> Result<T, Box<Response>> {
    payload.map(|Json(value)| value).map_err(|rejection| {
        let status = rejection.status();
        let (code, message) = match status {
            StatusCode::UNSUPPORTED_MEDIA_TYPE => (
                "unsupported_content_type",
                "The Content-Type header must be application/json.",
            ),
            StatusCode::PAYLOAD_TOO_LARGE => (
                "request_too_large",
                "The JSON request body exceeds the configured size limit.",
            ),
            StatusCode::UNPROCESSABLE_ENTITY => (
                "invalid_request_data",
                "The JSON body is missing fields or contains invalid values.",
            ),
            _ => ("invalid_json", "The request body must be valid JSON."),
        };
        warn!(error = %rejection, "invalid JSON request body");
        Box::new(rejection_response(status, code, message))
    })
}

fn validate_required(value: &str, field: &'static str) -> Result<(), SrunError> {
    if value.trim().is_empty() {
        return Err(SrunError::Validation {
            field,
            message: format!("{field} is required."),
        });
    }
    Ok(())
}

fn validate_optional_path(path: Option<&str>) -> Result<(), SrunError> {
    if path.is_some_and(|value| value.trim().is_empty()) {
        return Err(SrunError::Validation {
            field: "userinfo_path",
            message: "userinfo_path cannot be empty when provided.".to_string(),
        });
    }
    Ok(())
}

fn credentials<'a>(
    username: &'a Option<String>,
    password: &'a Option<String>,
) -> Result<Option<(&'a str, &'a str)>, SrunError> {
    match (username.as_deref(), password.as_deref()) {
        (None, None) => Ok(None),
        (Some(username), Some(password)) => {
            validate_required(username, "username")?;
            if password.is_empty() {
                return Err(SrunError::Validation {
                    field: "password",
                    message: "password is required.".to_string(),
                });
            }
            Ok(Some((username, password)))
        }
        _ => Err(SrunError::Validation {
            field: "credentials",
            message: "username and password must be provided together.".to_string(),
        }),
    }
}

pub async fn health() -> impl IntoResponse {
    Json(ApiResponse::ok("ok"))
}

pub async fn status(
    State(service): State<AppState>,
    query: std::result::Result<Query<StatusQuery>, QueryRejection>,
) -> Response {
    let Query(q) = match query {
        Ok(query) => query,
        Err(rejection) => {
            warn!(error = %rejection, "invalid status query");
            return rejection_response(
                StatusCode::BAD_REQUEST,
                "invalid_query",
                "The interface query parameter is required.",
            );
        }
    };
    if let Err(error) = validate_required(&q.interface, "interface") {
        return error_response(error);
    }
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
    payload: std::result::Result<Json<LocalLoginRequest>, JsonRejection>,
) -> Response {
    let req = match parse_json(payload) {
        Ok(req) => req,
        Err(response) => return *response,
    };
    if let Err(error) = validate_required(&req.interface, "interface")
        .and_then(|_| validate_optional_path(req.userinfo_path.as_deref()))
    {
        return error_response(error);
    }
    let creds = match credentials(&req.username, &req.password) {
        Ok(creds) => creds,
        Err(error) => return error_response(error),
    };
    if creds.is_some() && req.userinfo_path.is_some() {
        return error_response(SrunError::Validation {
            field: "userinfo_path",
            message: "userinfo_path cannot be combined with manual credentials.".to_string(),
        });
    }
    match service
        .login_local(&req.interface, creds, req.userinfo_path.as_deref())
        .await
    {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::ok(result))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn logout_local(
    State(service): State<AppState>,
    payload: std::result::Result<Json<LocalLogoutRequest>, JsonRejection>,
) -> Response {
    let req = match parse_json(payload) {
        Ok(req) => req,
        Err(response) => return *response,
    };
    if let Err(error) = validate_required(&req.interface, "interface") {
        return error_response(error);
    }
    match service.logout_local(&req.interface).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::<()>::ok_empty())).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn status_macvlan(
    State(service): State<AppState>,
    payload: std::result::Result<Json<MacvlanStatusRequest>, JsonRejection>,
) -> Response {
    let req = match parse_json(payload) {
        Ok(req) => req,
        Err(response) => return *response,
    };
    if let Err(error) = validate_required(&req.parent_interface, "parent_interface") {
        return error_response(error);
    }
    let mac = match parse_mac(&req.mac_address) {
        Ok(m) => m,
        Err(e) => return error_response(e),
    };
    match service
        .get_status_macvlan(&req.parent_interface, &mac)
        .await
    {
        Ok(s) => (StatusCode::OK, Json(ApiResponse::ok(s))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn login_macvlan(
    State(service): State<AppState>,
    payload: std::result::Result<Json<MacvlanLoginRequest>, JsonRejection>,
) -> Response {
    let req = match parse_json(payload) {
        Ok(req) => req,
        Err(response) => return *response,
    };
    if let Err(error) = validate_required(&req.parent_interface, "parent_interface")
        .and_then(|_| validate_optional_path(req.userinfo_path.as_deref()))
    {
        return error_response(error);
    }
    let mac = match parse_mac(&req.mac_address) {
        Ok(m) => m,
        Err(e) => return error_response(e),
    };

    let creds = match credentials(&req.username, &req.password) {
        Ok(creds) => creds,
        Err(error) => return error_response(error),
    };
    if creds.is_some() && req.userinfo_path.is_some() {
        return error_response(SrunError::Validation {
            field: "userinfo_path",
            message: "userinfo_path cannot be combined with manual credentials.".to_string(),
        });
    }
    match service
        .login_macvlan(
            &req.parent_interface,
            &mac,
            creds,
            req.userinfo_path.as_deref(),
        )
        .await
    {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::ok(result))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn logout_macvlan(
    State(service): State<AppState>,
    payload: std::result::Result<Json<MacvlanLogoutRequest>, JsonRejection>,
) -> Response {
    let req = match parse_json(payload) {
        Ok(req) => req,
        Err(response) => return *response,
    };
    if let Err(error) = validate_required(&req.parent_interface, "parent_interface") {
        return error_response(error);
    }
    let mac = match parse_mac(&req.mac_address) {
        Ok(m) => m,
        Err(e) => return error_response(e),
    };
    match service.logout_macvlan(&req.parent_interface, &mac).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::<()>::ok_empty())).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn login_random(
    State(service): State<AppState>,
    payload: std::result::Result<Json<RandomLoginRequest>, JsonRejection>,
) -> Response {
    let req = match parse_json(payload) {
        Ok(req) => req,
        Err(response) => return *response,
    };
    if let Err(error) = validate_required(&req.parent_interface, "parent_interface")
        .and_then(|_| validate_optional_path(req.userinfo_path.as_deref()))
    {
        return error_response(error);
    }

    match service
        .login_random(
            &req.parent_interface,
            req.count,
            req.userinfo_path.as_deref(),
        )
        .await
    {
        Ok(results) => (StatusCode::OK, Json(ApiResponse::ok(results))).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<()>::err_body(ApiErrorBody::new(
            "route_not_found",
            "The requested API route does not exist.",
        ))),
    )
        .into_response()
}

pub async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(ApiResponse::<()>::err_body(ApiErrorBody::new(
            "method_not_allowed",
            "This HTTP method is not supported for the requested route.",
        ))),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::credentials;

    #[test]
    fn credentials_must_be_complete() {
        let username = Some("alice".to_string());
        let password = Some("secret".to_string());
        assert_eq!(
            credentials(&username, &password).unwrap(),
            Some(("alice", "secret"))
        );
        assert!(credentials(&username, &None).is_err());
        assert!(credentials(&None, &password).is_err());
        assert!(credentials(&Some("  ".to_string()), &password).is_err());
    }
}
