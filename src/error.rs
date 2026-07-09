use std::net::Ipv4Addr;

#[derive(thiserror::Error, Debug)]
pub enum SrunError {
    #[error("portal request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("invalid JSON in portal response: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid IP address in portal response: {0}")]
    IpParse(#[from] std::net::AddrParseError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("netlink error: {0}")]
    Netlink(#[from] rtnetlink::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("credentials file error: {0}")]
    UserData(String),

    #[error("configured credentials store error: {0}")]
    CredentialStore(String),

    #[error("invalid {field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },

    #[error("DHCP failed: {0}")]
    Dhcp(String),

    #[error("DHCP {phase} timed out after {attempts} attempts")]
    DhcpTimeout { phase: &'static str, attempts: u32 },

    #[error("DHCP request was rejected: {0}")]
    DhcpRejected(String),

    #[error("failed to build a network packet")]
    PacketBuild,

    #[error("invalid JSONP response from portal")]
    JsonpParse,

    #[error("invalid portal response: {0}")]
    PortalResponse(String),

    #[error("portal returned HTTP status {0}")]
    PortalHttpStatus(u16),

    #[error("network interface '{0}' was not found")]
    InterfaceNotFound(String),

    #[error("authentication failed ({error}): {message}")]
    AuthFailed { error: String, message: String },

    #[error("MAC {mac} is already online as {user}")]
    AlreadyOnline { mac: String, user: String },

    #[error("IP address mismatch: DHCP={dhcp}, portal={portal}")]
    IpMismatch { dhcp: Ipv4Addr, portal: Ipv4Addr },

    #[error("no user is currently online")]
    NoUserOnline,

    #[error("interactive prompt error: {0}")]
    Inquire(#[from] inquire::InquireError),

    #[error("invalid MAC address: {0}")]
    InvalidMac(String),

    #[error("another network operation is already in progress")]
    OperationBusy,

    #[error("{operation} completed, but temporary-interface cleanup failed: {details}")]
    CleanupFailed {
        operation: &'static str,
        details: String,
    },

    #[error("{operation} failed ({primary}); temporary-interface cleanup also failed: {cleanup}")]
    OperationAndCleanupFailed {
        operation: &'static str,
        primary: String,
        cleanup: String,
    },
}

pub type Result<T> = std::result::Result<T, SrunError>;

impl SrunError {
    /// Convert an application error to the most accurate HTTP status available.
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::AuthFailed { .. } => StatusCode::UNAUTHORIZED,
            Self::AlreadyOnline { .. } => StatusCode::CONFLICT,
            Self::NoUserOnline | Self::OperationBusy | Self::IpMismatch { .. } => {
                StatusCode::CONFLICT
            }
            Self::InterfaceNotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidMac(_) | Self::Validation { .. } | Self::UserData(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::Request(error) if error.is_timeout() => StatusCode::GATEWAY_TIMEOUT,
            Self::DhcpTimeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::Request(_)
            | Self::Json(_)
            | Self::IpParse(_)
            | Self::JsonpParse
            | Self::PortalResponse(_)
            | Self::PortalHttpStatus(_)
            | Self::Dhcp(_)
            | Self::DhcpRejected(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable machine-readable code returned by the REST API.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Request(error) if error.is_timeout() => "portal_timeout",
            Self::Request(error) if error.status().is_some() => "portal_http_error",
            Self::Request(_) => "portal_unavailable",
            Self::PortalHttpStatus(_) => "portal_http_error",
            Self::Json(_) | Self::IpParse(_) | Self::JsonpParse | Self::PortalResponse(_) => {
                "invalid_portal_response"
            }
            Self::Io(_) | Self::Netlink(_) | Self::PacketBuild => "network_operation_failed",
            Self::Config(_) => "invalid_server_configuration",
            Self::UserData(_) => "invalid_credentials_file",
            Self::CredentialStore(_) => "credential_store_unavailable",
            Self::Validation { .. } => "validation_failed",
            Self::DhcpTimeout { .. } => "dhcp_timeout",
            Self::DhcpRejected(_) => "dhcp_rejected",
            Self::Dhcp(_) => "dhcp_failed",
            Self::InterfaceNotFound(_) => "interface_not_found",
            Self::AuthFailed { .. } => "authentication_failed",
            Self::AlreadyOnline { .. } => "already_online",
            Self::IpMismatch { .. } => "ip_mismatch",
            Self::NoUserOnline => "not_online",
            Self::Inquire(_) => "interactive_prompt_failed",
            Self::InvalidMac(_) => "invalid_mac_address",
            Self::OperationBusy => "operation_in_progress",
            Self::CleanupFailed { .. } => "cleanup_failed_after_success",
            Self::OperationAndCleanupFailed { .. } => "operation_failed_cleanup_incomplete",
        }
    }

    /// Safe, actionable copy for API clients. Internal diagnostics stay in logs.
    pub fn public_message(&self) -> String {
        match self {
            Self::Request(error) if error.is_timeout() => {
                "The Srun portal did not respond in time.".to_string()
            }
            Self::Request(error) if error.status().is_some() => {
                "The Srun portal returned an HTTP error.".to_string()
            }
            Self::Request(_) => "The Srun portal could not be reached.".to_string(),
            Self::PortalHttpStatus(_) => "The Srun portal returned an HTTP error.".to_string(),
            Self::Json(_) | Self::IpParse(_) | Self::JsonpParse | Self::PortalResponse(_) => {
                "The Srun portal returned an invalid response.".to_string()
            }
            Self::Io(_) | Self::Netlink(_) | Self::PacketBuild => {
                "The server could not complete the network operation.".to_string()
            }
            Self::Config(_) => "The server configuration is invalid.".to_string(),
            Self::UserData(_) => {
                "The selected credentials file is unavailable or invalid.".to_string()
            }
            Self::CredentialStore(_) => {
                "The server's configured credentials file is unavailable or invalid.".to_string()
            }
            Self::Validation { message, .. } => message.clone(),
            Self::Dhcp(message) => format!("DHCP failed: {message}"),
            Self::DhcpTimeout { phase, attempts } => {
                format!("DHCP {phase} timed out after {attempts} attempts.")
            }
            Self::DhcpRejected(message) => format!("DHCP rejected the request: {message}"),
            Self::AuthFailed { message, .. } => message.clone(),
            Self::CleanupFailed { operation, .. } => format!(
                "The {operation} completed, but the temporary network interface could not be removed. Do not retry automatically; check the server logs and network state."
            ),
            Self::OperationAndCleanupFailed { operation, .. } => format!(
                "The {operation} failed, and the temporary network interface could not be removed. Check the server logs and network state before retrying."
            ),
            _ => self.to_string(),
        }
    }

    pub fn field(&self) -> Option<&'static str> {
        match self {
            Self::Validation { field, .. } => Some(field),
            Self::InvalidMac(_) => Some("mac_address"),
            Self::UserData(_) => Some("userinfo_path"),
            _ => None,
        }
    }

    pub fn is_server_error(&self) -> bool {
        self.status_code().is_server_error()
    }
}
