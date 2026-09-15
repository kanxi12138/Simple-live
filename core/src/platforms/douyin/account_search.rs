use crate::platforms::common::request_limit::LimitedRequest;
// Official Douyin web user-search extension; separate from the Dart live-search baseline.
use serde::Serialize;
use serde_json::Value;

use crate::platforms::common::http_client::HttpClient;
use super::{search::bootstrap_search_context, web_api::DEFAULT_USER_AGENT};

const ACCOUNT_SEARCH_URL: &str = "https://www.douyin.com/aweme/v1/web/discover/search/";
const PAGE_SIZE: u32 = 10;
const LIVE_STATUS: i64 = 2;
const ENDED_STATUS: i64 = 4;

/// An account identity is distinct from its optional live room identifiers.
#[derive(Debug, Serialize)]
pub struct DouyinAccountItem {
    pub account_id: String,
    pub sec_uid: Option<String>,
    pub douyin_id: Option<String>,
    pub nickname: String,
    pub avatar: String,
    pub follower_count: Option<u64>,
    pub live_status: &'static str,
    pub room_id: Option<String>,
    pub web_rid: Option<String>,
}

/// Reads a nonzero string/number identifier without treating a user ID as a room ID.
pub(super) fn identifier(value: Option<&Value>) -> Option<String> {
    let text = match value? {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty() && text != "0").then_some(text)
}

/// Reads only follower counts, never audience or like counts; missing data remains unknown.
pub(super) fn follower_count(user: &Value) -> Option<u64> {
    let value = user.get("follower_count").or_else(|| user.pointer("/follow_info/follower_count"))?;
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn room_data(user: &Value) -> Option<Value> {
    match user.get("room_data")? {
        Value::Object(_) => user.get("room_data").cloned(),
        Value::String(text) if !text.is_empty() => match serde_json::from_str(text) {
            Ok(room) => Some(room),
            Err(_) => { log::warn!("Diagnostic: account_search.rs:48 (details omitted)"); None }
        },
        _ => None,
    }
}

fn live_status(user: &Value, room: Option<&Value>) -> &'static str {
    match room.and_then(|value| value.get("status")).and_then(Value::as_i64) {
        Some(LIVE_STATUS) => "live",
        Some(ENDED_STATUS) => "offline",
        _ if user.get("room_id_str").and_then(Value::as_str) == Some("0")
            || user.get("room_id").and_then(Value::as_u64) == Some(0) => "offline",
        _ => "unknown",
    }
}

fn parse_account(entry: &Value) -> Result<DouyinAccountItem, String> {
    let user = entry.get("user_info").ok_or("抖音账号结果缺少用户信息")?;
    let account_id = identifier(user.get("uid")).ok_or("抖音账号结果缺少账号标识")?;
    let room = room_data(user);
    let room_id = identifier(user.get("room_id_str")).or_else(|| identifier(user.get("room_id")));
    Ok(DouyinAccountItem {
        account_id,
        sec_uid: identifier(user.get("sec_uid")),
        douyin_id: identifier(user.get("unique_id")).or_else(|| identifier(user.get("short_id"))),
        nickname: user.get("nickname").and_then(Value::as_str).unwrap_or("").to_string(),
        avatar: user.pointer("/avatar_thumb/url_list/0").and_then(Value::as_str).unwrap_or("").to_string(),
        follower_count: follower_count(user),
        live_status: live_status(user, room.as_ref()),
        web_rid: room.as_ref().and_then(|value| identifier(value.pointer("/owner/web_rid"))),
        room_id,
    })
}

fn parse_accounts(payload: &Value) -> Result<Vec<DouyinAccountItem>, String> {
    if let Some(code) = payload.get("status_code").and_then(Value::as_i64).filter(|code| *code != 0) {
        let message = payload.get("status_msg").and_then(Value::as_str).unwrap_or("平台拒绝了搜索请求");
        return Err(format!("抖音账号搜索失败（{code}）：{message}"));
    }
    let list = payload.get("user_list").and_then(Value::as_array).ok_or("抖音账号搜索响应缺少用户列表")?;
    let mut accounts = Vec::new();
    for entry in list {
        match parse_account(entry) {
            Ok(account) => accounts.push(account),
            Err(_) => log::warn!("Diagnostic: account_search.rs:92 (details omitted)"),
        }
    }
    if !list.is_empty() && accounts.is_empty() { return Err("抖音账号结果无法解析".to_string()); }
    Ok(accounts)
}

/// Searches official user results by name or Douyin ID with runtime cookies.
/// Returns accounts including offline users; HTTP/platform/schema failures remain errors.
#[tauri::command]
pub async fn search_douyin_accounts(
    keyword: String, page: u32, cookie: Option<String>,
) -> Result<Vec<DouyinAccountItem>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() { return Ok(Vec::new()); }
    let client = HttpClient::new_direct_connection()?;
    let (cookie, webid) = bootstrap_search_context(&client, keyword, cookie.as_deref()).await?;
    let offset = page.saturating_sub(1).saturating_mul(PAGE_SIZE).to_string();
    let response = client.inner.get(ACCOUNT_SEARCH_URL)
        .query(&[
            ("device_platform", "webapp"), ("aid", "6383"), ("channel", "channel_pc_web"),
            ("search_channel", "aweme_user_web"), ("keyword", keyword), ("search_source", "normal_search"),
            ("query_correct_type", "1"), ("is_filter_search", "0"), ("from_group_id", ""),
            ("disable_rs", "0"), ("offset", offset.as_str()), ("count", "10"),
            ("need_filter_settings", "1"), ("list_type", "single"),
            ("update_version_code", "170400"), ("pc_client_type", "1"), ("pc_libra_divert", "Linux"),
            ("support_h265", "1"), ("support_dash", "0"), ("cpu_core_num", "8"),
            ("version_code", "170400"), ("version_name", "17.4.0"), ("cookie_enabled", "true"),
            ("browser_language", "zh-CN"), ("browser_platform", "Linux aarch64"),
            ("browser_name", "QQBrowser"), ("browser_version", "19.7.6764.400"),
            ("browser_online", "true"), ("engine_name", "Blink"), ("engine_version", "116.0.5845.97"),
            ("os_name", "Windows"), ("os_version", "10"), ("device_memory", "8"),
            ("platform", "PC"), ("webid", webid.as_str()),
        ][..])
        .header("User-Agent", DEFAULT_USER_AGENT).header("Cookie", cookie)
        .header("Accept", "application/json, text/plain, */*")
        .header("Referer", format!("https://www.douyin.com/search/{}?type=user", urlencoding::encode(keyword)))
        .send_limited().await.map_err(|error| format!("抖音账号请求失败: {error}"))?
        .error_for_status().map_err(|error| format!("抖音账号请求失败: {error}"))?;
    let payload: Value = response.json().await.map_err(|error| format!("抖音账号响应解析失败: {error}"))?;
    parse_accounts(&payload)
}
