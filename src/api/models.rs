use crate::error::SrunError;
use serde::{Deserialize, Serialize};

// ---- Requests ----

#[derive(Debug, Deserialize)]
pub struct LocalLoginRequest {
    pub interface: String,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Path to a JSON file with [{ "username", "password" }] entries.
    /// Used only when `username`/`password` are not provided.
    pub userinfo_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LocalLogoutRequest {
    pub interface: String,
}

#[derive(Debug, Deserialize)]
pub struct MacvlanLoginRequest {
    pub parent_interface: String,
    pub mac_address: String,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Path to a JSON file with [{ "username", "password" }] entries.
    /// Used only when `username`/`password` are not provided.
    pub userinfo_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MacvlanLogoutRequest {
    pub parent_interface: String,
    pub mac_address: String,
}

#[derive(Debug, Deserialize)]
pub struct RandomLoginRequest {
    pub parent_interface: String,
    pub count: u32,
    /// Path to a JSON file with [{ "username", "password" }] entries.
    pub userinfo_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub interface: String,
}

#[derive(Debug, Deserialize)]
pub struct MacvlanStatusRequest {
    pub parent_interface: String,
    pub mac_address: String,
}

// ---- Responses ----

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<&'static str>,
}

impl ApiErrorBody {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
        }
    }
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
}

impl ApiResponse<()> {
    pub fn ok_empty() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }

    pub fn err(error: &SrunError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiErrorBody {
                code: error.code(),
                message: error.public_message(),
                field: error.field(),
            }),
        }
    }

    pub fn err_body(error: ApiErrorBody) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct InterfaceInfo {
    pub index: u32,
    pub name: String,
}
