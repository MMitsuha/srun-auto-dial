pub mod base64;
pub mod utils;
pub mod xencode;

use crate::config::Config;
use crate::error::{Result, SrunError};
use reqwest::Client;
use serde_json::Value;
use std::net::Ipv4Addr;
use tracing::{debug, trace};

pub struct UserInfo {
    pub ip: Ipv4Addr,
    pub online_user: Option<String>,
    pub online_mac: Option<String>,
}

/// Srun portal protocol client, configured with portal URL and AC ID.
pub struct SrunClient {
    base_url: String,
    ac_id: String,
}

impl SrunClient {
    pub fn new(config: &Config) -> Self {
        Self {
            base_url: config.portal_url.clone(),
            ac_id: config.ac_id.clone(),
        }
    }

    pub async fn get_userinfo(&self, client: &Client, callback: &str) -> Result<UserInfo> {
        let url = format!("{}/cgi-bin/rad_user_info", self.base_url);
        let ts = utils::timestamp_millis().to_string();

        debug!(url = %url, "requesting userinfo");

        let resp = client
            .get(&url)
            .query(&[("callback", callback), ("_", &ts)])
            .send()
            .await?
            .text()
            .await?;

        trace!(response = %resp, "userinfo response");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let json: Value = serde_json::from_str(json_str)?;

        let ip_str = json["online_ip"]
            .as_str()
            .ok_or_else(|| SrunError::AuthFailed {
                error: "missing_ip".to_string(),
                message: "online_ip field missing from userinfo response".to_string(),
            })?;
        let ip: Ipv4Addr = ip_str.parse()?;

        let online_user = json["user_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let online_mac = json["user_mac"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        debug!(ip = %ip, user = ?online_user, "userinfo result");
        Ok(UserInfo {
            ip,
            online_user,
            online_mac,
        })
    }

    pub async fn get_challenge(
        &self,
        client: &Client,
        callback: &str,
        username: &str,
        ip: Ipv4Addr,
    ) -> Result<String> {
        let url = format!("{}/cgi-bin/get_challenge", self.base_url);
        let ts = utils::timestamp_millis().to_string();

        debug!(url = %url, username = %username, "requesting challenge");

        let resp = client
            .get(&url)
            .query(&[
                ("callback", callback),
                ("username", username),
                ("ip", &ip.to_string()),
                ("_", &ts),
            ])
            .send()
            .await?
            .text()
            .await?;

        trace!(response = %resp, "challenge response");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let json: Value = serde_json::from_str(json_str)?;

        let challenge = json["challenge"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SrunError::AuthFailed {
                error: "missing_challenge".to_string(),
                message: "challenge field missing or empty".to_string(),
            })?
            .to_string();

        debug!(challenge = %challenge, "received challenge");
        Ok(challenge)
    }

    pub async fn login(
        &self,
        client: &Client,
        callback: &str,
        username: &str,
        password: &str,
        ip: Ipv4Addr,
        challenge: &str,
    ) -> Result<()> {
        let url = format!("{}/cgi-bin/srun_portal", self.base_url);
        let ts = utils::timestamp_millis().to_string();

        let password_md5 = utils::get_md5(password, challenge);
        debug!(password_md5 = %password_md5, "computed password hash");

        let info_raw = format!(
            r#"{{"username":"{}","password":"{}","ip":"{}","acid":"{}","enc_ver":"srun_bx1"}}"#,
            username, password, ip, self.ac_id
        );
        let info_encoded = base64::get_base64(&xencode::get_xencode(&info_raw, challenge));
        let info_encoded = format!("{{SRBX1}}{}", info_encoded);

        let checksum_input = format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            challenge,
            username,
            challenge,
            password_md5,
            challenge,
            self.ac_id,
            challenge,
            ip,
            challenge,
            "200",
            challenge,
            "1",
            challenge,
            info_encoded,
        );
        let checksum = utils::get_sha1(&checksum_input);
        debug!(checksum = %checksum, "computed login checksum");

        let password_field = format!("{{MD5}}{}", password_md5);
        let ip_str = ip.to_string();

        let resp = client
            .get(&url)
            .query(&[
                ("callback", callback),
                ("action", "login"),
                ("username", username),
                ("password", &password_field),
                ("os", "Windows 10"),
                ("name", "Windows"),
                ("double_stack", "0"),
                ("chksum", &checksum),
                ("info", &info_encoded),
                ("ac_id", &self.ac_id),
                ("ip", &ip_str),
                ("n", "200"),
                ("type", "1"),
                ("_", &ts),
            ])
            .send()
            .await?
            .text()
            .await?;

        trace!(response = %resp, "login response");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let json: Value = serde_json::from_str(json_str)?;

        let error_field = json["error"].as_str().unwrap_or("");
        if error_field != "ok" {
            return Err(SrunError::AuthFailed {
                error: error_field.to_string(),
                message: json["error_msg"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string(),
            });
        }

        Ok(())
    }

    pub async fn logout(
        &self,
        client: &Client,
        callback: &str,
        username: &str,
        ip: Ipv4Addr,
    ) -> Result<()> {
        let url = format!("{}/cgi-bin/rad_user_dm", self.base_url);
        let ts = utils::timestamp_secs();
        let tsm = ts * 1000;

        let sign_input = format!("{}{}{}{}{}", ts, username, ip, "1", ts);
        let sign = utils::get_sha1(&sign_input);
        debug!(sign = %sign, "computed logout sign");

        let ts_str = ts.to_string();
        let tsm_str = tsm.to_string();
        let ip_str = ip.to_string();

        let resp = client
            .get(&url)
            .query(&[
                ("callback", callback),
                ("ip", &ip_str),
                ("username", username),
                ("time", &ts_str),
                ("unbind", "1"),
                ("sign", &sign),
                ("_", &tsm_str),
            ])
            .send()
            .await?
            .text()
            .await?;

        trace!(response = %resp, "logout response");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let json: Value = serde_json::from_str(json_str)?;

        let error_field = json["error"].as_str().unwrap_or("");
        if error_field != "ok" {
            return Err(SrunError::AuthFailed {
                error: error_field.to_string(),
                message: json["error_msg"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string(),
            });
        }

        Ok(())
    }
}
