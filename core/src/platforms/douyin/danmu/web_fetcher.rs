use crate::platforms::common::http_client::HttpClient;
use crate::platforms::douyin::web_api::{
    fetch_room_data, normalize_douyin_live_id, DouyinRoomData, DEFAULT_COOKIE, DEFAULT_USER_AGENT,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::signature::generate_ms_token;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DouyinFollowListRoomInfo {
    pub web_rid: String,
    pub nickname: String,
    pub room_name: String,
    pub avatar_url: String,
    pub status: i32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ResolvedRoomInfo {
    pub web_rid: Option<String>,
    pub room_id: String,
    pub nickname: String,
    pub room_name: String,
    pub avatar_url: String,
    pub status: i32,
}

pub struct DouyinLiveWebFetcher {
    pub live_id: String,
    pub room_id: Option<String>,
    resolved_info: Option<ResolvedRoomInfo>,
    pub user_agent: String,
    pub http_client: HttpClient,
    pub(crate) _ws_stream: Option<Arc<Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    pub dy_cookie: Option<String>,
    pub user_unique_id: Option<String>,
}

impl DouyinLiveWebFetcher {
    pub fn new(live_id: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let http_client = HttpClient::new_direct_connection()
            .map_err(|e| format!("Failed to create direct connection HttpClient: {}", e))?;
        let normalized_live_id = normalize_douyin_live_id(live_id);

        Ok(DouyinLiveWebFetcher {
            live_id: normalized_live_id,
            room_id: None,
            resolved_info: None,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            http_client,
            _ws_stream: None,
            dy_cookie: None,
            user_unique_id: None,
        })
    }

    async fn resolve_room_info(
        &mut self,
    ) -> Result<ResolvedRoomInfo, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(info) = &self.resolved_info {
            return Ok(info.clone());
        }

        let live_id = self.live_id.clone();
        let cookies = self.dy_cookie.as_deref();
        match fetch_room_data(&self.http_client, &live_id, cookies).await {
            Ok(DouyinRoomData { room }) => {
                let room_id = room
                    .get("id_str")
                    .and_then(|v| v.as_str())
                    .or_else(|| room.get("id").and_then(|v| v.as_str()))
                    .unwrap_or(&live_id)
                    .to_string();
                let status = room.get("status").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
                let room_name = room
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let owner = room.get("owner").cloned().unwrap_or_else(|| Value::Null);
                let anchor = room.get("anchor").cloned().unwrap_or_else(|| Value::Null);
                let nickname = owner
                    .get("nickname")
                    .and_then(|v| v.as_str())
                    .or_else(|| anchor.get("nickname").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                let avatar_url = owner
                    .get("avatar_thumb")
                    .or_else(|| anchor.get("avatar_thumb"))
                    .and_then(|a| a.get("url_list"))
                    .and_then(|ul| ul.get(0))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let web_rid_val = owner
                    .get("web_rid")
                    .and_then(|v| v.as_str())
                    .or_else(|| anchor.get("web_rid").and_then(|v| v.as_str()))
                    .unwrap_or(&live_id)
                    .to_string();

                let info = ResolvedRoomInfo {
                    web_rid: Some(web_rid_val),
                    room_id: room_id.clone(),
                    nickname,
                    room_name,
                    avatar_url,
                    status,
                };
                self.room_id = Some(room_id);
                self.resolved_info = Some(info.clone());
                Ok(info)
            }
            Err(api_err) => Err(Box::new(std::io::Error::other(format!(
                "Failed to resolve via web enter API: {}",
                api_err
            )))),
        }
    }

    pub async fn collect_cookies_and_ids(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resolved = self.resolve_room_info().await?;
        let web_rid = resolved
            .web_rid
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| self.live_id.clone());
        let homepage_url = "https://live.douyin.com/";
        let room_url = format!("https://live.douyin.com/{web_rid}");
        let mut cookie_map = parse_cookie_header(DEFAULT_COOKIE);

        let head_resp = self
            .http_client
            .inner
            .head(homepage_url)
            .header("User-Agent", &self.user_agent)
            .header("Referer", "https://live.douyin.com/")
            .header("Authority", "live.douyin.com")
            .send()
            .await?;
        merge_cookie_map_from_response(&mut cookie_map, &head_resp);

        let home_resp = self
            .http_client
            .inner
            .get(homepage_url)
            .header("User-Agent", &self.user_agent)
            .header("Referer", "https://live.douyin.com/")
            .send()
            .await?;
        merge_cookie_map_from_response(&mut cookie_map, &home_resp);

        let initial_cookie_header = build_cookie_header(&cookie_map);
        let room_resp = self
            .http_client
            .inner
            .get(&room_url)
            .header("User-Agent", &self.user_agent)
            .header("Referer", "https://live.douyin.com/")
            .header("Cookie", &initial_cookie_header)
            .send()
            .await?;
        merge_cookie_map_from_response(&mut cookie_map, &room_resp);
        let room_html = room_resp.text().await.unwrap_or_default();

        if !cookie_map.contains_key("msToken") {
            cookie_map.insert("msToken".to_string(), generate_ms_token(107));
        }

        if self.room_id.as_deref().unwrap_or_default().is_empty() {
            if let Some(room_id) = extract_first_value(
                &room_html,
                &[
                    "\"roomId\":\"",
                    "\"room_id\":\"",
                    "\\\"roomId\\\":\\\"",
                    "\\\"room_id\\\":\\\"",
                ],
            ) {
                self.room_id = Some(room_id);
            }
        }

        let fallback_uid = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        let user_unique_id = cookie_map
            .get("s_v_web_id")
            .cloned()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                extract_first_value(
                    &room_html,
                    &[
                        "\"user_unique_id\":\"",
                        "\"user_unique_id_str\":\"",
                        "\\\"user_unique_id\\\":\\\"",
                        "\\\"user_unique_id_str\\\":\\\"",
                    ],
                )
            })
            .or_else(|| cookie_map.get("ttwid").cloned())
            .unwrap_or(fallback_uid);

        self.dy_cookie = Some(build_cookie_header(&cookie_map));
        self.user_unique_id = Some(user_unique_id);
        Ok(())
    }

    pub async fn fetch_room_details(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.collect_cookies_and_ids().await
    }

    pub async fn get_room_id(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(room_id) = &self.room_id {
            return Ok(room_id.clone());
        }
        self.resolve_room_info().await?;
        self.room_id
            .clone()
            .ok_or_else(|| "room_id not set after cookie collection".into())
    }

    pub async fn get_user_unique_id(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(uid) = &self.user_unique_id {
            return Ok(uid.clone());
        }
        self.collect_cookies_and_ids().await?;
        self.user_unique_id
            .clone()
            .ok_or_else(|| "user_unique_id not set after cookie collection".into())
    }

    pub async fn get_dy_cookie(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(cookie) = &self.dy_cookie {
            return Ok(cookie.clone());
        }
        self.collect_cookies_and_ids().await?;
        self.dy_cookie
            .clone()
            .ok_or_else(|| "cookie not set after cookie collection".into())
    }

    #[allow(dead_code)]
    pub async fn get_room_status(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let room_id_val = self.get_room_id().await?;
        let dy_cookie = self.get_dy_cookie().await?;
        let user_unique_id = self.get_user_unique_id().await?;

        let ms_token = dy_cookie
            .split(';')
            .filter_map(|kv| {
                let kv = kv.trim();
                kv.strip_prefix("msToken=").map(ToString::to_string)
            })
            .next()
            .unwrap_or_default();

        let base_url = "https://live.douyin.com/webcast/room/web/enter/?aid=6383&app_name=douyin_web&live_id=1&device_platform=web&language=zh-CN&cookie_enabled=true";
        let url = if ms_token.is_empty() {
            format!(
                "{}&room_id={}&user_unique_id={}",
                base_url, room_id_val, user_unique_id
            )
        } else {
            format!(
                "{}&room_id={}&msToken={}&user_unique_id={}",
                base_url, room_id_val, ms_token, user_unique_id
            )
        };

        if let Err(e) = self.http_client.insert_header(USER_AGENT, &self.user_agent) {
            return Err(std::io::Error::other(format!(
                "Failed to set USER_AGENT header: {}",
                e
            ))
            .into());
        }

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9"));
        headers.insert(
            REFERER,
            HeaderValue::from_str(&format!("https://live.douyin.com/{}", self.live_id))
                .unwrap_or_else(|_| HeaderValue::from_static("https://live.douyin.com")),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("cookie"),
            HeaderValue::from_str(&dy_cookie).map_err(|e| {
                std::io::Error::other(format!("Invalid Cookie header: {}", e))
            })?,
        );

        let data: serde_json::Value = self
            .http_client
            .get_json_with_headers(&url, Some(headers))
            .await
            .map_err(|e| std::io::Error::other(format!("Failed to get room status: {}", e)))?;

        if let Some(room_info) = data.get("data").and_then(|v| v.get("room")) {
            let status = room_info.get("status").and_then(|s| s.as_i64()).unwrap_or(-1);
            let nickname = room_info
                .get("owner")
                .and_then(|v| v.get("nickname"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            println!(
                "[Douyin] room_status fetched: room_id={}, nickname={}, status={}",
                room_id_val, nickname, status
            );
        }
        Ok(())
    }
}

fn parse_cookie_header(cookie_header: &str) -> BTreeMap<String, String> {
    let mut cookie_map = BTreeMap::new();
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            upsert_cookie_if_needed(&mut cookie_map, key.trim(), value.trim());
        }
    }
    cookie_map
}

fn merge_cookie_map_from_response(
    cookie_map: &mut BTreeMap<String, String>,
    response: &reqwest::Response,
) {
    for cookie in response.cookies() {
        upsert_cookie_if_needed(cookie_map, cookie.name(), cookie.value());
    }
    for value in response.headers().get_all("set-cookie").iter() {
        if let Ok(cookie_line) = value.to_str() {
            let first = cookie_line.split(';').next().unwrap_or("").trim();
            if let Some((key, val)) = first.split_once('=') {
                upsert_cookie_if_needed(cookie_map, key.trim(), val.trim());
            }
        }
    }
}

fn upsert_cookie_if_needed(cookie_map: &mut BTreeMap<String, String>, key: &str, value: &str) {
    if matches!(
        key,
        "ttwid" | "msToken" | "s_v_web_id" | "__ac_nonce" | "__ac_signature" | "tt_scid"
    ) && !value.is_empty()
    {
        cookie_map.insert(key.to_string(), value.to_string());
    }
}

fn build_cookie_header(cookie_map: &BTreeMap<String, String>) -> String {
    cookie_map
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn extract_first_value(source: &str, markers: &[&str]) -> Option<String> {
    markers.iter().find_map(|marker| {
        let start = source.find(marker)?;
        let suffix = &source[start + marker.len()..];
        let end = suffix.find('"').or_else(|| suffix.find('\\'))?;
        let value = suffix[..end].trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

#[tauri::command]
pub async fn fetch_douyin_room_info(live_id: String) -> Result<DouyinFollowListRoomInfo, String> {
    println!(
        "[fetch_douyin_room_info] Fetching details for web_id: {}",
        live_id
    );
    let normalized_id = normalize_douyin_live_id(&live_id);

    let http_client = HttpClient::new_direct_connection()
        .map_err(|e| format!("Failed to create direct connection HttpClient: {}", e))?;

    let DouyinRoomData { room } = fetch_room_data(&http_client, &normalized_id, None)
        .await
        .map_err(|e| format!("Failed to fetch Douyin room data: {}", e))?;

    let web_rid = crate::platforms::douyin::douyin_streamer_detail::extract_web_rid(&room)
        .unwrap_or_else(|| normalized_id.clone());
    let nickname = crate::platforms::douyin::douyin_streamer_detail::extract_anchor_name(&room)
        .unwrap_or_else(|| format!("主播{}", web_rid));
    let room_name = room
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let avatar_url =
        crate::platforms::douyin::douyin_streamer_detail::extract_avatar(&room).unwrap_or_default();
    let status = room
        .get("status")
        .and_then(|v| v.as_i64())
        .unwrap_or_default() as i32;

    Ok(DouyinFollowListRoomInfo {
        web_rid,
        nickname,
        room_name,
        avatar_url,
        status,
    })
}
