pub mod base64;
pub mod utils;
pub mod xencode;

use crate::config::Config;
use crate::error::{Result, SrunError};
use reqwest::Client;
use serde::Deserialize;
use std::net::Ipv4Addr;
use tracing::{debug, trace};

const MAX_PORTAL_RESPONSE_BYTES: usize = 1024 * 1024;

pub struct UserInfo {
    pub ip: Ipv4Addr,
    pub online_user: Option<String>,
    pub online_mac: Option<String>,
}

#[derive(Deserialize)]
struct UserInfoResponse {
    online_ip: String,
    #[serde(default)]
    user_name: Option<String>,
    #[serde(default)]
    user_mac: Option<String>,
}

#[derive(Deserialize)]
struct ChallengeResponse {
    #[serde(default)]
    challenge: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_msg: Option<String>,
}

#[derive(Deserialize)]
struct PortalActionResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_msg: Option<String>,
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

        let resp = read_portal_response(
            client
                .get(&url)
                .query(&[("callback", callback), ("_", &ts)])
                .send()
                .await
                .map_err(sanitize_request_error)?,
        )
        .await?;

        trace!(endpoint = "userinfo", "portal response received");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let response: UserInfoResponse = serde_json::from_str(json_str)?;
        let ip: Ipv4Addr = response.online_ip.parse()?;

        let online_user = response.user_name.filter(|value| !value.trim().is_empty());
        let online_mac = response.user_mac.filter(|value| !value.trim().is_empty());

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

        let resp = read_portal_response(
            client
                .get(&url)
                .query(&[
                    ("callback", callback),
                    ("username", username),
                    ("ip", &ip.to_string()),
                    ("_", &ts),
                ])
                .send()
                .await
                .map_err(sanitize_request_error)?,
        )
        .await?;

        trace!(endpoint = "challenge", "portal response received");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let response: ChallengeResponse = serde_json::from_str(json_str)?;
        let challenge = response
            .challenge
            .filter(|value| !value.is_empty())
            .ok_or_else(|| match response.error {
                Some(error) if !error.is_empty() && error != "ok" => SrunError::AuthFailed {
                    error,
                    message: response.error_msg.unwrap_or_else(|| {
                        "The portal rejected the challenge request.".to_string()
                    }),
                },
                _ => SrunError::PortalResponse("challenge field is missing or empty".to_string()),
            })?;

        debug!("received portal challenge");
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
        let info_raw = login_info_json(username, password, ip, &self.ac_id);
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

        let password_field = format!("{{MD5}}{}", password_md5);
        let ip_str = ip.to_string();

        let resp = read_portal_response(
            client
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
                .await
                .map_err(sanitize_request_error)?,
        )
        .await?;

        trace!(endpoint = "login", "portal response received");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let response: PortalActionResponse = serde_json::from_str(json_str)?;
        let error_field = response.error.ok_or_else(|| {
            SrunError::PortalResponse("login response is missing the error field".to_string())
        })?;
        if error_field != "ok" {
            return Err(SrunError::AuthFailed {
                error: error_field,
                message: response
                    .error_msg
                    .unwrap_or_else(|| "The portal rejected the login request.".to_string()),
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

        let ts_str = ts.to_string();
        let tsm_str = tsm.to_string();
        let ip_str = ip.to_string();

        let resp = read_portal_response(
            client
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
                .await
                .map_err(sanitize_request_error)?,
        )
        .await?;

        trace!(endpoint = "logout", "portal response received");

        let json_str = utils::extract_json_from_jsonp(&resp, callback)?;
        let response: PortalActionResponse = serde_json::from_str(json_str)?;
        let error_field = response.error.ok_or_else(|| {
            SrunError::PortalResponse("logout response is missing the error field".to_string())
        })?;
        if error_field != "ok" {
            return Err(SrunError::AuthFailed {
                error: error_field,
                message: response
                    .error_msg
                    .unwrap_or_else(|| "The portal rejected the logout request.".to_string()),
            });
        }

        Ok(())
    }
}

async fn read_portal_response(mut response: reqwest::Response) -> Result<String> {
    let status = response.status();
    if !status.is_success() {
        return Err(SrunError::PortalHttpStatus(status.as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PORTAL_RESPONSE_BYTES as u64)
    {
        return Err(SrunError::PortalResponse(format!(
            "response exceeds {MAX_PORTAL_RESPONSE_BYTES} bytes"
        )));
    }

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(sanitize_request_error)? {
        if body.len() + chunk.len() > MAX_PORTAL_RESPONSE_BYTES {
            return Err(SrunError::PortalResponse(format!(
                "response exceeds {MAX_PORTAL_RESPONSE_BYTES} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body)
        .map_err(|error| SrunError::PortalResponse(format!("response is not UTF-8: {error}")))
}

fn sanitize_request_error(error: reqwest::Error) -> SrunError {
    SrunError::Request(error.without_url())
}

fn login_info_json(username: &str, password: &str, ip: Ipv4Addr, ac_id: &str) -> String {
    serde_json::json!({
        "username": username,
        "password": password,
        "ip": ip,
        "acid": ac_id,
        "enc_ver": "srun_bx1",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::login_info_json;
    use serde_json::Value;
    use std::net::Ipv4Addr;

    #[test]
    fn login_info_escapes_credentials_as_json() {
        let encoded = login_info_json(
            "user\"name",
            "line\\break\n密码",
            Ipv4Addr::new(192, 0, 2, 10),
            "1",
        );
        let value: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["username"], "user\"name");
        assert_eq!(value["password"], "line\\break\n密码");
        assert_eq!(value["ip"], "192.0.2.10");
    }
}
