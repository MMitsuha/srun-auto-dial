use crate::error::{Result, SrunError};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
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
    /// Load and validate configuration. A missing implicit `srun.toml` uses
    /// defaults; every other read or parse failure is reported explicitly.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let mut config = match path {
            Some(p) => {
                let content = std::fs::read_to_string(p).map_err(|e| {
                    SrunError::Config(format!(
                        "failed to read configuration file '{}': {e}",
                        p.display()
                    ))
                })?;
                toml::from_str(&content).map_err(|e| {
                    SrunError::Config(format!(
                        "failed to parse configuration file '{}': {e}",
                        p.display()
                    ))
                })?
            }
            None => match std::fs::read_to_string("srun.toml") {
                Ok(content) => toml::from_str(&content)
                    .map_err(|e| SrunError::Config(format!("failed to parse 'srun.toml': {e}")))?,
                Err(error) if error.kind() == ErrorKind::NotFound => Self::default(),
                Err(error) => {
                    return Err(SrunError::Config(format!(
                        "failed to read 'srun.toml': {error}"
                    )));
                }
            },
        };

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&mut self) -> Result<()> {
        self.portal_url = self.portal_url.trim().trim_end_matches('/').to_string();
        let portal = reqwest::Url::parse(&self.portal_url)
            .map_err(|e| SrunError::Config(format!("portal_url is not a valid URL: {e}")))?;
        if !matches!(portal.scheme(), "http" | "https") || portal.host_str().is_none() {
            return Err(SrunError::Config(
                "portal_url must be an absolute http:// or https:// URL".to_string(),
            ));
        }
        if !portal.username().is_empty() || portal.password().is_some() {
            return Err(SrunError::Config(
                "portal_url must not contain embedded credentials".to_string(),
            ));
        }
        if portal.query().is_some() || portal.fragment().is_some() {
            return Err(SrunError::Config(
                "portal_url must not contain a query string or fragment".to_string(),
            ));
        }
        self.ac_id = self.ac_id.trim().to_string();
        self.server.host = self.server.host.trim().to_string();
        if self.ac_id.is_empty() {
            return Err(SrunError::Config("ac_id cannot be empty".to_string()));
        }
        if self.server.host.is_empty() {
            return Err(SrunError::Config("server.host cannot be empty".to_string()));
        }
        if self
            .server
            .api_key
            .as_ref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(SrunError::Config(
                "server.api_key cannot be empty when provided".to_string(),
            ));
        }
        if self
            .server
            .api_key
            .as_ref()
            .is_some_and(|key| key != key.trim())
        {
            return Err(SrunError::Config(
                "server.api_key cannot have leading or trailing whitespace".to_string(),
            ));
        }
        if self
            .userinfo_path
            .as_ref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err(SrunError::Config(
                "userinfo_path cannot be empty when provided".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn validates_and_normalizes_portal_url() {
        let mut config = Config {
            portal_url: " https://portal.example.edu:8443/ ".to_string(),
            ac_id: " 1 ".to_string(),
            server: super::ServerConfig {
                host: " 127.0.0.1 ".to_string(),
                ..super::ServerConfig::default()
            },
            ..Config::default()
        };
        config.validate().unwrap();
        assert_eq!(config.portal_url, "https://portal.example.edu:8443");
        assert_eq!(config.ac_id, "1");
        assert_eq!(config.server.host, "127.0.0.1");
    }

    #[test]
    fn rejects_invalid_configuration_values() {
        let mut config = Config {
            portal_url: "portal.example.edu".to_string(),
            ..Config::default()
        };
        assert!(config.validate().is_err());

        let mut config = Config {
            server: super::ServerConfig {
                api_key: Some("   ".to_string()),
                ..super::ServerConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }
}
