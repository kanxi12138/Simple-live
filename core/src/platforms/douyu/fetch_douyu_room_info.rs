use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::platforms::common::FollowHttpClient;

const DOUYU_REFERER_PREFIX: &str = "https://www.douyu.com/";
const DOUYU_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DouyuFollowInfo {
    room_id: String,
    room_name: Option<String>,
    nickname: Option<String>,
    avatar_url: Option<String>,
    video_loop: Option<i64>,
    show_status: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct BetardRoomInfo {
    room_id: Option<Value>,
    room_name: Option<String>,
    nickname: Option<String>,
    owner_name: Option<String>,
    avatar_mid: Option<String>,
    owner_avatar: Option<String>,
    show_status: Option<Value>,
    #[serde(rename = "videoLoop")]
    video_loop: Option<Value>,
}

#[derive(Deserialize, Debug)]
struct BetardResponse {
    room: Option<BetardRoomInfo>,
}

#[tauri::command]
pub async fn fetch_douyu_room_info(
    room_id: String,
    follow_http: State<'_, FollowHttpClient>,
) -> Result<DouyuFollowInfo, String> {
    let headers = build_request_headers(&room_id)?;
    let response_text = follow_http
        .0
        .inner
        .get(format!("https://www.douyu.com/betard/{room_id}"))
        .headers(headers)
        .send()
        .await
        .map_err(|error| format!("Network request failed for room {room_id}: {error}"))?
        .text()
        .await
        .map_err(|error| format!("Failed reading Douyu room response for {room_id}: {error}"))?;

    if let Ok(parsed) = serde_json::from_str::<BetardResponse>(&response_text) {
        if let Some(room) = parsed.room {
            return Ok(DouyuFollowInfo {
                room_id: room
                    .room_id
                    .as_ref()
                    .and_then(value_to_string)
                    .unwrap_or_else(|| room_id.clone()),
                room_name: room.room_name,
                nickname: room.nickname.or(room.owner_name),
                avatar_url: room.avatar_mid.or(room.owner_avatar),
                video_loop: room.video_loop.as_ref().and_then(value_to_i64),
                show_status: room.show_status.as_ref().and_then(value_to_i64),
            });
        }
    }

    let fallback_room_id = resolve_room_id_from_mobile_page(&follow_http, &room_id).await?;
    Ok(DouyuFollowInfo {
        room_id: fallback_room_id,
        room_name: None,
        nickname: None,
        avatar_url: None,
        video_loop: Some(0),
        show_status: Some(1),
    })
}

fn build_request_headers(room_id: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("zh-CN,zh;q=0.9"),
    );
    headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert(
        "Referer",
        HeaderValue::from_str(&format!("{DOUYU_REFERER_PREFIX}{room_id}"))
            .map_err(|error| format!("Invalid referer header for room {room_id}: {error}"))?,
    );
    headers.insert("User-Agent", HeaderValue::from_static(DOUYU_USER_AGENT));
    Ok(headers)
}

async fn resolve_room_id_from_mobile_page(
    follow_http: &FollowHttpClient,
    room_id: &str,
) -> Result<String, String> {
    let mobile_url = format!("https://m.douyu.com/{room_id}");
    let html = follow_http
        .0
        .inner
        .get(&mobile_url)
        .header("Referer", &mobile_url)
        .header("User-Agent", DOUYU_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("Failed loading Douyu mobile page for {room_id}: {error}"))?
        .text()
        .await
        .map_err(|error| format!("Failed reading Douyu mobile page for {room_id}: {error}"))?;

    let rid_regex =
        Regex::new(r#""rid":(\d{1,12})"#).map_err(|error| format!("Invalid rid regex: {error}"))?;
    rid_regex
        .captures(&html)
        .and_then(|captures| captures.get(1))
        .map(|matched| matched.as_str().to_string())
        .ok_or_else(|| format!("Missing rid in Douyu mobile room page for {room_id}"))
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.to_string()),
        _ => None,
    }
}
