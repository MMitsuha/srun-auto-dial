use serde::{Deserialize, Serialize};

// ---- Requests ----

#[derive(Debug, Deserialize)]
pub struct LocalLoginRequest {
    pub interface: String,
    pub username: Option<String>,
    pub password: Option<String>,
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
}

#[derive(Debug, Deserialize)]
pub struct MacvlanLogoutRequest {
    pub parent_interface: String,
}

#[derive(Debug, Deserialize)]
pub struct RandomLoginRequest {
    pub parent_interface: String,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub interface: String,
}

// ---- Responses ----

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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

    pub fn err(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct InterfaceInfo {
    pub index: u32,
    pub name: String,
}
