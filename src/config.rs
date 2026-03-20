use crate::error::{Result, SrunError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_portal_url")]
    pub portal_url: String,

    #[serde(default = "default_ac_id")]
    pub ac_id: String,

    #[serde(default)]
    pub userinfo_path: Option<String>,

    #[serde(default)]
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_portal_url() -> String {
    "http://portal.hdu.edu.cn".to_string()
}

fn default_ac_id() -> String {
    "1".to_string()
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            portal_url: default_portal_url(),
            ac_id: default_ac_id(),
            userinfo_path: None,
            server: ServerConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            api_key: None,
        }
    }
}

impl Config {
    /// Load config from file path if provided, otherwise use defaults.
    /// If the file doesn't exist, fall back to defaults silently.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        match path {
            Some(p) => {
                let content = std::fs::read_to_string(p).map_err(|e| {
                    SrunError::Config(format!("无法读取配置文件 {}: {}", p.display(), e))
                })?;
                toml::from_str(&content)
                    .map_err(|e| SrunError::Config(format!("配置文件解析失败: {}", e)))
            }
            None => {
                // Try default path "srun.toml" silently
                if let Ok(content) = std::fs::read_to_string("srun.toml") {
                    toml::from_str(&content)
                        .map_err(|e| SrunError::Config(format!("配置文件解析失败: {}", e)))
                } else {
                    Ok(Self::default())
                }
            }
        }
    }

    /// Derive the Host header value from portal_url
    pub fn portal_host(&self) -> &str {
        self.portal_url
            .strip_prefix("http://")
            .or_else(|| self.portal_url.strip_prefix("https://"))
            .unwrap_or(&self.portal_url)
    }
}
