use crate::platforms::common::http_client::HttpClient;
use crate::platforms::douyin::a_bogus::generate_a_bogus;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, COOKIE, REFERER, USER_AGENT};
use serde_json::Value;

// Align UA with the working Douyin Rust sample to keep a_bogus inputs consistent.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.5845.97 Safari/537.36 Core/1.116.567.400 QQBrowser/19.7.6764.400";

#[derive(Debug, Clone)]
pub struct DouyinRoomData {
    pub room: Value,
}

async fn fetch_room_from_api(
    http_client: &HttpClient,
    web_id: &str,
    cookies: Option<&str>,
) -> Result<DouyinRoomData, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
    headers.insert(REFERER, HeaderValue::from_str(&format!("https://live.douyin.com/{web_id}")).map_err(|e| format!("Invalid Referer: {e}"))?);
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    headers.insert(COOKIE, HeaderValue::from_str(cookies.unwrap_or("")).map_err(|e| format!("Invalid cookie header value: {}", e))?);

    let params = vec![
        ("aid", "6383"),
        ("app_name", "douyin_web"),
        ("live_id", "1"),
        ("device_platform", "web"),
        ("language", "zh-CN"),
        ("browser_language", "zh-CN"),
        ("browser_platform", "Win32"),
        ("browser_name", "Chrome"),
        ("browser_version", "125.0.0.0"),
        ("web_rid", web_id),
        ("msToken", ""),
    ];
    let query = serde_urlencoded::to_string(&params)
        .map_err(|e| format!("Failed to encode Douyin enter params: {}", e))?;
    let sign = generate_a_bogus(&query, DEFAULT_USER_AGENT);
    let api = format!(
        "https://live.douyin.com/webcast/room/web/enter/?{}&a_bogus={}",
        query,
        sign
    );
    let json: Value = http_client
        .inner
        .get(&api)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Failed to request Douyin web enter API: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse Douyin web enter response: {}", e))?;

    let room = json
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(|arr| arr.get(0))
        .cloned()
        .ok_or_else(|| "Douyin web enter API did not return room data".to_string())?;

    let anchor_name = json
        .get("data")
        .and_then(|d| d.get("user"))
        .and_then(|u| u.get("nickname"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut room_mut = room;
    if let Some(name) = anchor_name {
        if let Some(obj) = room_mut.as_object_mut() {
            obj.insert("anchor_name".to_string(), Value::String(name));
        }
    }
    Ok(DouyinRoomData { room: room_mut })
}

/// Normalize user input into a Douyin web_id. Supports raw IDs and full URLs such as
/// `https://live.douyin.com/123456` or `https://www.douyin.com/follow/live/123456`.
pub fn normalize_douyin_live_id(id_or_url: &str) -> String {
    let trimmed = id_or_url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Prefer explicit room/query parameters if present.
    if let Some(qpos) = trimmed.find('?') {
        let query = &trimmed[qpos + 1..];
        for kv in query.split('&') {
            if let Some(val) = kv
                .strip_prefix("room_id=")
                .or_else(|| kv.strip_prefix("roomId="))
                .or_else(|| kv.strip_prefix("web_rid="))
                .or_else(|| kv.strip_prefix("webId="))
            {
                let cleaned = val
                    .split(['&', '#'])
                    .find(|s| !s.is_empty())
                    .unwrap_or(val);
                if !cleaned.is_empty() {
                    return cleaned.to_string();
                }
            }
        }
    }

    // Handle any douyin.com URL (live.douyin.com, www.douyin.com/follow/live/xxx, etc.).
    if let Some(pos) = trimmed.find("douyin.com/") {
        let start = pos + "douyin.com/".len();
        let remainder = &trimmed[start..];
        let path_only = remainder.split(['?', '#']).next().unwrap_or(remainder);
        if let Some(segment) = path_only
            .rsplit('/')
            .find(|segment| !segment.is_empty())
        {
            return segment
                .split(['?', '&', '#'])
                .find(|s| !s.is_empty())
                .unwrap_or(segment)
                .to_string();
        }
    }

    // Fallback: strip trailing query/hash from raw input.
    trimmed
        .split(['?', '&', '#'])
        .find(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

pub async fn fetch_room_data(
    http_client: &HttpClient,
    raw_id: &str,
    cookies: Option<&str>,
) -> Result<DouyinRoomData, String> {
    let web_id = normalize_douyin_live_id(raw_id);
    if web_id.is_empty() || !web_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("抖音房间号必须是数字".to_string());
    }
    let cookie = match cookies.filter(|value| !value.trim().is_empty()) {
        Some(cookie) => cookie.to_string(),
        None => super::room_page::runtime_cookie(&http_client.inner, &web_id).await.map_err(|error| error.to_string())?,
    };
    let mut actual_web_id = web_id.clone();
    if web_id.len() > 16 {
        let room = super::room_page::reflow_room(&http_client.inner, &web_id, &cookie).await.map_err(|error| error.to_string())?;
        if room["status"].as_i64() != Some(4) { return Ok(DouyinRoomData { room }); }
        actual_web_id = room.pointer("/owner/web_rid").and_then(Value::as_str)
            .ok_or("抖音房间已失效且未返回固定房间号")?.to_string();
    }
    match fetch_room_from_api(http_client, &actual_web_id, Some(&cookie)).await {
        Ok(room) => Ok(room),
        Err(error) => {
            log::warn!("Douyin enter API failed; reading page: {error}");
            let room = super::room_page::html_room(&http_client.inner, &actual_web_id, &cookie)
                .await.map_err(|fallback| format!("抖音房间获取失败: {error}; 页面回退: {fallback}"))?;
            Ok(DouyinRoomData { room })
        }
    }
}

