use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, REFERER, USER_AGENT};
use serde::Serialize;
use serde_json::Value;
use tauri::command;

use crate::platforms::common::http_client::HttpClient;
use crate::platforms::common::FollowHttpClient;

use super::web_api::DEFAULT_USER_AGENT;

const DOUYIN_SEARCH_REFERER_BASE: &str = "https://www.douyin.com/search/";
const DOUYIN_SEARCH_URL: &str = "https://www.douyin.com/aweme/v1/web/live/search/";

#[derive(Debug, Clone, Serialize)]
pub struct DouyinSearchLiveItem {
    pub web_rid: String,
    pub room_id: String,
    pub title: String,
    pub nickname: String,
    pub avatar: String,
    pub is_live: bool,
    pub status: i32,
    pub total_user_str: String,
}

fn build_search_cookie_from_response(response: &reqwest::Response) -> BTreeMap<String, String> {
    let mut cookies = BTreeMap::new();
    for cookie in response.cookies() {
        let name = cookie.name().to_string();
        if matches!(
            name.as_str(),
            "ttwid" | "msToken" | "s_v_web_id" | "__ac_nonce" | "__ac_signature" | "tt_scid"
        ) {
            cookies.insert(name, cookie.value().to_string());
        }
    }
    cookies
}

fn fallback_webid() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("738{}{}", millis, millis % 10)
}

async fn bootstrap_search_context(
    client: &HttpClient,
    keyword: &str,
) -> Result<(String, String), String> {
    let referer = format!("{}{}?type=live", DOUYIN_SEARCH_REFERER_BASE, urlencoding::encode(keyword));
    let response = client
        .inner
        .get(&referer)
        .header(USER_AGENT, DEFAULT_USER_AGENT)
        .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
        .send()
        .await
        .map_err(|e| format!("Failed to bootstrap Douyin search cookies: {}", e))?;

    let cookie_map = build_search_cookie_from_response(&response);
    let cookie_header = if cookie_map.is_empty() {
        super::web_api::DEFAULT_COOKIE.to_string()
    } else {
        cookie_map
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    };

    let webid = cookie_map
        .get("s_v_web_id")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(fallback_webid);

    Ok((cookie_header, webid))
}

fn parse_search_item(item: &Value) -> Option<DouyinSearchLiveItem> {
    let raw = item.get("lives")?.get("rawdata")?.as_str()?;
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let owner = parsed.get("owner")?;
    let status = parsed.get("status").and_then(|v| v.as_i64()).unwrap_or_default() as i32;
    let web_rid = owner
        .get("web_rid")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())?
        .to_string();
    let room_id = parsed
        .get("id_str")
        .and_then(|v| v.as_str())
        .or_else(|| parsed.get("id").and_then(|v| v.as_str()))
        .unwrap_or(&web_rid)
        .to_string();
    let avatar = owner
        .get("avatar_thumb")
        .and_then(|v| v.get("url_list"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Some(DouyinSearchLiveItem {
        web_rid,
        room_id,
        title: parsed
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        nickname: owner
            .get("nickname")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        avatar,
        is_live: status == 2,
        status,
        total_user_str: parsed
            .get("stats")
            .and_then(|v| v.get("total_user_str"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

#[command]
pub async fn search_douyin_live_rooms(
    keyword: String,
    page: u32,
    _follow_http: tauri::State<'_, FollowHttpClient>,
) -> Result<Vec<DouyinSearchLiveItem>, String> {
    let trimmed_keyword = keyword.trim();
    if trimmed_keyword.is_empty() {
        return Ok(Vec::new());
    }

    let client = HttpClient::new_direct_connection()
        .map_err(|e| format!("Failed to create Douyin search client: {}", e))?;
    let (cookie_header, webid) = bootstrap_search_context(&client, trimmed_keyword).await?;

    let offset = page.saturating_sub(1) * 10;
    let offset_string = offset.to_string();
    let query = vec![
        ("device_platform", "webapp".to_string()),
        ("aid", "6383".to_string()),
        ("channel", "channel_pc_web".to_string()),
        ("search_channel", "aweme_live".to_string()),
        ("keyword", trimmed_keyword.to_string()),
        ("search_source", "switch_tab".to_string()),
        ("query_correct_type", "1".to_string()),
        ("is_filter_search", "0".to_string()),
        ("from_group_id", "".to_string()),
        ("offset", offset_string),
        ("count", "10".to_string()),
        ("pc_client_type", "1".to_string()),
        ("version_code", "170400".to_string()),
        ("version_name", "17.4.0".to_string()),
        ("cookie_enabled", "true".to_string()),
        ("screen_width", "1980".to_string()),
        ("screen_height", "1080".to_string()),
        ("browser_language", "zh-CN".to_string()),
        ("browser_platform", "Win32".to_string()),
        ("browser_name", "Edge".to_string()),
        ("browser_version", "125.0.0.0".to_string()),
        ("browser_online", "true".to_string()),
        ("engine_name", "Blink".to_string()),
        ("engine_version", "125.0.0.0".to_string()),
        ("os_name", "Windows".to_string()),
        ("os_version", "10".to_string()),
        ("cpu_core_num", "12".to_string()),
        ("device_memory", "8".to_string()),
        ("platform", "PC".to_string()),
        ("downlink", "10".to_string()),
        ("effective_type", "4g".to_string()),
        ("round_trip_time", "100".to_string()),
        ("webid", webid),
    ];
    let request = client
        .inner
        .get(DOUYIN_SEARCH_URL)
        .query(&query)
        .header("Authority", "www.douyin.com")
        .header(ACCEPT, "application/json, text/plain, */*")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
        .header(COOKIE, cookie_header)
        .header(REFERER, format!("{}{}?type=live", DOUYIN_SEARCH_REFERER_BASE, urlencoding::encode(trimmed_keyword)))
        .header("sec-ch-ua", "\"Microsoft Edge\";v=\"125\", \"Chromium\";v=\"125\", \"Not.A/Brand\";v=\"24\"")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"Windows\"")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin")
        .header(USER_AGENT, DEFAULT_USER_AGENT);

    let response_text = request
        .send()
        .await
        .map_err(|e| format!("Douyin live search request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read Douyin live search response: {}", e))?;

    if response_text.trim().is_empty() || response_text.trim().eq_ignore_ascii_case("blocked") {
        return Err("Douyin live search was blocked by upstream".to_string());
    }

    let payload: Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse Douyin live search response: {}", e))?;

    let items = payload
        .get("data")
        .and_then(|v| v.as_array())
        .map(|entries| entries.iter().filter_map(parse_search_item).collect())
        .unwrap_or_else(Vec::new);

    Ok(items)
}
