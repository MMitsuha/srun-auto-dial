use crate::config::Config;
use crate::error::SrunError;
use hmac::{Hmac, Mac};
use md5::Md5;
use rand::{Rng, rng};
use reqwest::header::{HeaderMap, HeaderValue};
use sha1::{Digest, Sha1};
use std::time::{SystemTime, UNIX_EPOCH};

/// Build default HTTP headers, deriving Host/Referer from config.
pub fn build_default_headers(config: &Config) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("Accept", HeaderValue::from_static("text/javascript, application/javascript, application/ecmascript, application/x-ecmascript, */*; q=0.01"));
    h.insert("Accept-Encoding", HeaderValue::from_static("gzip, deflate, br, zstd"));
    h.insert("Accept-Language", HeaderValue::from_static("en-US,en;q=0.9"));
    h.insert("Connection", HeaderValue::from_static("keep-alive"));

    if let Ok(v) = HeaderValue::from_str(config.portal_host()) {
        h.insert("Host", v);
    }

    let referer = format!(
        "{}/srun_portal_pc?ac_id={}&theme=pro",
        config.portal_url, config.ac_id
    );
    if let Ok(v) = HeaderValue::from_str(&referer) {
        h.insert("Referer", v);
    }

    h.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    h.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    h.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    h.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0"));
    h.insert("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"));
    h.insert("sec-ch-ua", HeaderValue::from_static("\"Microsoft Edge\";v=\"141\", \"Not?A_Brand\";v=\"8\", \"Chromium\";v=\"141\""));
    h.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    h.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Windows\""));
    h
}

/// Generate a jQuery-style JSONP callback name.
pub fn generate_jsonp_callback() -> String {
    let mut rng = rng();
    let random_part: u64 = rng.random_range(100_000_000_000_000..999_999_999_999_999);
    let ts = timestamp_millis();
    format!("jQuery11240{}_{}", random_part, ts)
}

/// Extract JSON body from a JSONP response like `callback({...})`.
pub fn extract_json_from_jsonp<'a>(
    jsonp: &'a str,
    callback: &str,
) -> crate::error::Result<&'a str> {
    let prefix = format!("{}(", callback);
    jsonp
        .strip_prefix(&prefix)
        .and_then(|s| s.strip_suffix(')'))
        .ok_or(SrunError::JsonpParse)
}

/// HMAC-MD5 of password with challenge token as key.
pub fn get_md5(password: &str, token: &str) -> String {
    let mut mac =
        Hmac::<Md5>::new_from_slice(token.as_bytes()).expect("HMAC-MD5 accepts any key length");
    mac.update(password.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// SHA1 hash of a string.
pub fn get_sha1(value: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

/// Current timestamp in milliseconds since UNIX epoch.
pub fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_millis() as u64
}

/// Current timestamp in seconds since UNIX epoch.
pub fn timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs()
}
