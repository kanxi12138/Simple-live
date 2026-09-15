use crate::platforms::common::request_limit::LimitedRequest;
// Ported from dart_simple_live ba828e6, huya_site.dart (GPL-3.0).
// Copyright xiaoyaocz and contributors; see THIRD_PARTY_NOTICES.md.
use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::platforms::common::FollowHttpClient;
use super::cdn_token::{sign_anti_code, PLAY_USER_AGENT};

const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 11; Pixel 5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.91 Mobile Safari/537.36 Edg/117.0.0.0";
const LIVE_STATUS: i64 = 2;

/// A page-provided quality and CDN combination, with its freshly signed URL.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HuyaUnifiedStreamEntry {
    pub quality: String,
    pub bit_rate: i32,
    pub url: String,
    pub line: String,
}

/// Keeps the existing command response and adds actual lines and playback headers.
#[derive(Clone, Debug, Serialize)]
pub struct HuyaUnifiedResponse {
    pub title: Option<String>,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub introduction: Option<String>,
    #[serde(rename = "profileRoom")]
    pub profile_room: Option<String>,
    pub is_live: bool,
    pub flv_tx_urls: Vec<HuyaUnifiedStreamEntry>,
    pub selected_url: Option<String>,
    pub lines: Vec<String>,
    pub headers: BTreeMap<String, String>,
}

/// Parsed mobile room data shared by playback and danmaku parameter acquisition.
pub(super) struct RoomPage {
    pub data: Value,
    pub top_sid: i64,
    pub sub_sid: i64,
}

fn channel_id(html: &str, field: &str) -> Result<i64> {
    let expression = Regex::new(&format!(r#""{field}"\s*:\s*"?(\d+)"#))?;
    expression.captures(html).and_then(|capture| capture.get(1))
        .context(format!("虎牙页面缺少 {field}"))?.as_str().parse().map_err(Into::into)
}

/// Fetches the mobile room page for `room_id`; rejects HTTP and malformed page data.
pub(super) async fn fetch_room_page(client: &reqwest::Client, room_id: &str) -> Result<RoomPage> {
    if room_id.is_empty() || !room_id.chars().all(|character| character.is_ascii_alphanumeric()) {
        bail!("虎牙房间号无效");
    }
    let html = client.get(format!("https://m.huya.com/{room_id}"))
        .header("User-Agent", MOBILE_USER_AGENT).send_limited().await?
        .error_for_status()?.text().await?;
    let expression = Regex::new(r"(?s)window\.HNF_GLOBAL_INIT\s*=\s*(\{.*?\})\s*;?\s*</script>")?;
    let json = expression.captures(&html).and_then(|capture| capture.get(1))
        .context("虎牙页面缺少房间数据，可能受到平台限制")?.as_str();
    let functions = Regex::new(r"(?s)function\s*[^({]*\([^)]*\)\s*\{.*?\}")?;
    let data: Value = serde_json::from_str(&functions.replace_all(json, "\"\""))?;
    let live_status = data.pointer("/roomInfo/eLiveStatus").and_then(Value::as_i64)
        .context("虎牙页面缺少直播状态")?;
    let (top_sid, sub_sid) = if live_status == LIVE_STATUS {
        (channel_id(&html, "lChannelId")?, channel_id(&html, "lSubChannelId")?)
    } else { (0, 0) };
    Ok(RoomPage { data, top_sid, sub_sid })
}

fn string_field(data: &Value, path: &str) -> Option<String> {
    data.pointer(path).and_then(|value| match value {
        Value::String(text) => Some(html_escape::decode_html_entities(text).into_owned()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

fn build_entries(page: &RoomPage) -> Result<Vec<HuyaUnifiedStreamEntry>> {
    let live = page.data.pointer("/roomInfo/tLiveInfo/tLiveStreamInfo")
        .context("虎牙房间缺少直播流信息")?;
    let lines = live.pointer("/vStreamInfo/value").and_then(Value::as_array)
        .context("虎牙未返回线路")?;
    let qualities = live.pointer("/vBitRateInfo/value").and_then(Value::as_array)
        .context("虎牙未返回画质")?;
    let mut entries = Vec::new();
    let mut tokens = BTreeMap::new();
    for line in lines {
        let Some(base) = line["sFlvUrl"].as_str().filter(|value| !value.is_empty()) else { continue; };
        let stream = line["sStreamName"].as_str().context("虎牙线路缺少流名称")?;
        let cdn = line["sCdnType"].as_str().context("虎牙线路缺少 CDN 类型")?;
        if !tokens.contains_key(stream) {
            let token = line["sFlvAntiCode"].as_str().filter(|value| !value.is_empty())
                .context("虎牙公开页面未提供播放 Token，暂不可用")?;
            tokens.insert(stream.to_string(), sign_anti_code(stream, page.top_sid, &token)?);
        }
        let token = tokens.get(stream).context("虎牙 Token 未生成")?;
        let base = if base.starts_with("//") { format!("https:{base}") } else { base.to_string() };
        for quality in qualities {
            let name = quality["sDisplayName"].as_str().context("虎牙画质缺少名称")?;
            if name.contains("HDR") { continue; }
            let bit_rate = quality["iBitRate"].as_i64().context("虎牙画质缺少码率")? as i32;
            let ratio = if bit_rate > 0 { format!("&ratio={bit_rate}") } else { String::new() };
            entries.push(HuyaUnifiedStreamEntry {
                quality: name.to_string(), bit_rate, line: cdn.to_string(),
                url: format!("{base}/{stream}.flv?{token}&codec=264{ratio}"),
            });
        }
    }
    if entries.is_empty() { bail!("虎牙未返回可播放的画质和线路"); }
    Ok(entries)
}

/// Resolves metadata and stream options for `room_id`, selecting `quality` and `line`.
/// Unavailable preferences use the first returned option. HTTP and protocol errors propagate.
#[tauri::command]
pub async fn get_huya_unified_cmd(
    room_id: String,
    quality: Option<String>,
    line: Option<String>,
    follow_http: State<'_, FollowHttpClient>,
) -> Result<HuyaUnifiedResponse, String> {
    let client = &follow_http.0.inner;
    let page = fetch_room_page(client, &room_id).await.map_err(|error| error.to_string())?;
    let is_live = page.data.pointer("/roomInfo/eLiveStatus").and_then(Value::as_i64) == Some(LIVE_STATUS);
    let entries = if is_live {
        build_entries(&page).map_err(|error| error.to_string())?
    } else { Vec::new() };
    let mut lines = Vec::new();
    for entry in &entries {
        if !lines.contains(&entry.line) { lines.push(entry.line.clone()); }
    }
    let selected_line = line.as_ref().filter(|candidate| lines.contains(candidate)).or_else(|| lines.first());
    let selected = entries.iter().find(|entry| Some(&entry.line) == selected_line && Some(&entry.quality) == quality.as_ref())
        .or_else(|| entries.iter().find(|entry| Some(&entry.line) == selected_line));
    Ok(HuyaUnifiedResponse {
        title: string_field(&page.data, "/roomInfo/tLiveInfo/sIntroduction")
            .filter(|title| !title.is_empty()).or_else(|| string_field(&page.data, "/roomInfo/tLiveInfo/sRoomName")),
        nick: string_field(&page.data, "/roomInfo/tProfileInfo/sNick"),
        avatar: string_field(&page.data, "/roomInfo/tProfileInfo/sAvatar180"),
        introduction: string_field(&page.data, "/roomInfo/tLiveInfo/sIntroduction"),
        profile_room: string_field(&page.data, "/roomInfo/tLiveInfo/lProfileRoom"),
        is_live, selected_url: selected.map(|entry| entry.url.clone()),
        flv_tx_urls: entries, lines,
        headers: BTreeMap::from([("User-Agent".to_string(), PLAY_USER_AGENT.to_string())]),
    })
}
