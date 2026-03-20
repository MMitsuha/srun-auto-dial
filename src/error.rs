use std::net::Ipv4Addr;

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum SrunError {
    #[error("网络请求失败: {0}")]
    Request(#[from] reqwest::Error),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IP 解析失败: {0}")]
    IpParse(#[from] std::net::AddrParseError),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("Netlink 错误: {0}")]
    Netlink(#[from] rtnetlink::Error),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("DHCP 失败: {0}")]
    Dhcp(String),

    #[error("数据包构建失败")]
    PacketBuild,

    #[error("JSONP 解析失败")]
    JsonpParse,

    #[error("接口 {0} 不存在")]
    InterfaceNotFound(String),

    #[error("认证失败: {error} {message}")]
    AuthFailed { error: String, message: String },

    #[error("MAC {mac} 已在线, 用户: {user}")]
    AlreadyOnline { mac: String, user: String },

    #[error("IP 不匹配: DHCP={dhcp}, Portal={portal}")]
    IpMismatch { dhcp: Ipv4Addr, portal: Ipv4Addr },

    #[error("无用户在线")]
    NoUserOnline,

    #[error("未授权")]
    Unauthorized,

    #[error("TUI 交互错误: {0}")]
    Inquire(#[from] inquire::InquireError),

    #[error("MAC 地址格式错误: {0}")]
    InvalidMac(String),
}

pub type Result<T> = std::result::Result<T, SrunError>;

impl SrunError {
    /// Convert to HTTP status code for API responses
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::AuthFailed { .. } => StatusCode::UNAUTHORIZED,
            Self::AlreadyOnline { .. } => StatusCode::CONFLICT,
            Self::NoUserOnline => StatusCode::NOT_FOUND,
            Self::InterfaceNotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidMac(_) => StatusCode::BAD_REQUEST,
            Self::Config(_) => StatusCode::BAD_REQUEST,
            Self::JsonpParse => StatusCode::BAD_GATEWAY,
            Self::IpMismatch { .. } => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
