use chrono::Local;
use hmac::{Hmac, Mac};
use md5::Md5;
use rand::{Rng, rng};
use regex::Regex;
use reqwest::header::HeaderMap;
use sha1::{Digest, Sha1};

pub fn build_default_header() -> HeaderMap {
    let mut default_headers = HeaderMap::new();
    default_headers.insert("Accept", "text/javascript, application/javascript, application/ecmascript, application/x-ecmascript, */*; q=0.01".parse().unwrap());
    default_headers.insert(
        "Accept-Encoding",
        "gzip, deflate, br, zstd".parse().unwrap(),
    );
    default_headers.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
    default_headers.insert("Connection", "keep-alive".parse().unwrap());
    default_headers.insert("Host", "portal.hdu.edu.cn".parse().unwrap());
    default_headers.insert(
        "Referer",
        "http://portal.hdu.edu.cn/srun_portal_pc?ac_id=1&theme=pro"
            .parse()
            .unwrap(),
    );
    default_headers.insert("Sec-Fetch-Dest", "empty".parse().unwrap());
    default_headers.insert("Sec-Fetch-Mode", "cors".parse().unwrap());
    default_headers.insert("Sec-Fetch-Site", "same-origin".parse().unwrap());
    default_headers.insert("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0".parse().unwrap());
    default_headers.insert("X-Requested-With", "XMLHttpRequest".parse().unwrap());
    default_headers.insert(
        "sec-ch-ua",
        "\"Microsoft Edge\";v=\"141\", \"Not?A_Brand\";v=\"8\", \"Chromium\";v=\"141\""
            .parse()
            .unwrap(),
    );
    default_headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
    default_headers.insert("sec-ch-ua-platform", "\"Windows\"".parse().unwrap());
    default_headers
}

pub fn generate_jsonp_callback() -> String {
    let mut rng = rng();

    // jQuery 会生成一个 16~17 位左右的随机整数，这里模仿一下 jQuery11240595074804891746_1764496234439
    // jQuery11240191658346009159_1764642950420
    let random_part: u64 = rng.random_range(100_000_000_000_000..999_999_999_999_999);

    // 时间戳（毫秒）
    let ts = Local::now().timestamp_millis();

    format!("jQuery11240{}_{}", random_part, ts)
}

pub fn extract_json_from_jsonp<'a>(jsonp: &'a str, callback: &'a str) -> Option<&'a str> {
    // ^jQuery\d+_\d+  -> 匹配开头的 jQuery + 数字 + 下划线 + 数字
    // \(.*\)$           -> 匹配从第一个 '(' 到最后一个 ')' 的内容
    let re = Regex::new(&format!(r"^{}\((.*)\)$", regex::escape(callback))).unwrap();

    re.captures(jsonp)
        .and_then(|caps| caps.get(1).map(|m| m.as_str()))
}

pub fn get_md5(password: &str, token: &str) -> String {
    let mut mac = Hmac::<Md5>::new_from_slice(token.as_bytes()).unwrap();
    mac.update(password.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn get_sha1(value: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}
