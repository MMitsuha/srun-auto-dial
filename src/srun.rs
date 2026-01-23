pub mod base64;
pub mod utils;
pub mod xencode;

use chrono::Local;
use reqwest::Client;
use serde_json::Value;
use std::net::Ipv4Addr;

const URL_BASE: &str = "http://portal.hdu.edu.cn";

pub struct UserInfo {
    pub ip: Ipv4Addr,
    pub online_user: Option<String>,
    pub online_mac: Option<String>,
}

pub async fn get_userinfo(client: Client, callback: &str) -> Result<UserInfo, String> {
    let url = format!("{}{}", URL_BASE, "/cgi-bin/rad_user_info");
    let ts = Local::now().timestamp_millis();

    let resp = client
        .get(url)
        .query(&[("callback", callback), ("_", &ts.to_string())])
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Response text error: {}", e))?;

    let json_str = utils::extract_json_from_jsonp(&resp, callback)
        .ok_or_else(|| "Failed to extract JSON from JSONP".to_string())?;
    // println!("Extracted JSON: {}", json_str);
    let json: Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;
    let ip = json["online_ip"]
        .as_str()
        .unwrap_or("")
        .parse::<Ipv4Addr>()
        .map_err(|e| format!("IP parse error: {}", e))?;
    let online_user = json["user_name"].as_str().map(|s| s.to_string());
    let online_mac = json["user_mac"].as_str().map(|s| s.to_string());

    Ok(UserInfo {
        ip,
        online_user,
        online_mac,
    })
}

pub async fn get_challenge(
    client: Client,
    callback: &str,
    username: &str,
    ip: Ipv4Addr,
) -> Result<String, String> {
    let url = format!("{}{}", URL_BASE, "/cgi-bin/get_challenge");
    let ts = Local::now().timestamp_millis();

    let resp = client
        .get(url)
        .query(&[
            ("callback", callback),
            ("username", username),
            ("ip", &ip.to_string()),
            ("_", &ts.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Response text error: {}", e))?;

    let json_str = utils::extract_json_from_jsonp(&resp, callback)
        .ok_or_else(|| "Failed to extract JSON from JSONP".to_string())?;
    // println!("Extracted JSON: {}", json_str);
    let json: Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;
    let challenge = json["challenge"].as_str().unwrap_or("").to_string();

    Ok(challenge)
}

pub async fn login(
    client: Client,
    callback: &str,
    username: &str,
    password: &str,
    ip: Ipv4Addr,
    challenge: &str,
) -> Result<(), String> {
    let url = format!("{}{}", URL_BASE, "/cgi-bin/srun_portal");
    let ts = Local::now().timestamp_millis();

    let password_md5 = utils::get_md5(password, challenge);
    // println!("Password MD5: {}", password_md5);

    let info_raw = format!(
        r#"{{"username":"{}","password":"{}","ip":"{}","acid":"1","enc_ver":"srun_bx1"}}"#,
        username, password, ip
    );
    // println!("Info raw: {}", info_raw);
    let info_encoded = base64::get_base64(&xencode::get_xencode(info_raw.as_str(), challenge));
    let info_encoded = format!("{}{}", "{SRBX1}", info_encoded);
    // println!("Info encoded: {}", info_encoded);

    let checksum_input = format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        challenge,
        username,
        challenge,
        password_md5,
        challenge,
        "1",
        challenge,
        ip,
        challenge,
        "200",
        challenge,
        "1",
        challenge,
        info_encoded
    ); // token+self.username+token+hmd5+token+self.ac_id+token+self.ip+token+"200"+token+"1"+token+i
    // println!("Checksum input: {}", checksum_input);

    let checksum = utils::get_sha1(&checksum_input);
    // println!("Checksum: {}", checksum);

    let resp = client
        .get(url)
        .query(&[
            ("callback", callback),
            ("action", "login"),
            ("username", username),
            ("password", format!("{}{}", "{MD5}", password_md5).as_str()),
            ("os", "Windows 10"),
            ("name", "Windows"),
            ("double_stack", "0"),
            ("chksum", &checksum),
            ("info", &info_encoded),
            ("ac_id", "1"),
            ("ip", &ip.to_string()),
            ("n", "200"),
            ("type", "1"),
            ("_", &ts.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Response text error: {}", e))?;

    let json_str = utils::extract_json_from_jsonp(&resp, callback)
        .ok_or_else(|| "Failed to extract JSON from JSONP".to_string())?;
    // println!("Extracted JSON: {}", json_str);
    let json: Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;
    if json["error"].as_str().unwrap_or("") != "ok" {
        return Err(format!(
            "Login failed: {} {}",
            json["error"].as_str().unwrap_or("Unknown error"),
            json["error_msg"].as_str().unwrap_or("Unknown error")
        ));
    }

    Ok(())
}

pub async fn logout(
    client: Client,
    callback: &str,
    username: &str,
    ip: Ipv4Addr,
) -> Result<(), String> {
    let url = format!("{}{}", URL_BASE, "/cgi-bin/rad_user_dm");
    let ts = Local::now().timestamp();
    let tsm = ts * 1000;

    let sign = utils::get_sha1(format!("{}{}{}{}{}", ts, username, ip, "1", ts).as_str()); // time + self.online_user + self.ip + "1" + time

    let resp = client
        .get(url)
        .query(&[
            ("callback", callback),
            ("ip", &ip.to_string()),
            ("username", username),
            ("time", &ts.to_string()),
            ("unbind", "1"),
            ("sign", &sign),
            ("_", &tsm.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Response text error: {}", e))?;

    let json_str = utils::extract_json_from_jsonp(&resp, callback)
        .ok_or_else(|| "Failed to extract JSON from JSONP".to_string())?;
    // println!("Extracted JSON: {}", json_str);
    let json: Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;
    if json["error"].as_str().unwrap_or("") != "ok" {
        return Err(format!(
            "Logout failed: {}",
            json["error_msg"].as_str().unwrap_or("Unknown error")
        ));
    }

    Ok(())
}
