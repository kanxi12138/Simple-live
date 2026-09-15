use crate::platforms::common::request_limit::LimitedRequest;
// Ported from dart_simple_live ba828e6 douyin_site.dart (GPL-3.0).
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::Value;

use super::web_api::DEFAULT_USER_AGENT;

/// Fetches runtime cookies for `web_id`; a user cookie is handled by the caller.
/// Returns HTTP errors instead of substituting a fixed session credential.
pub(super) async fn runtime_cookie(client: &Client, web_id: &str) -> Result<String> {
    let room_url = format!("https://live.douyin.com/{web_id}");
    let mut last_error = "抖音未返回运行时 ttwid Cookie".to_string();
    // HEAD can return cookies with 404; intermittent 503 responses need a page fallback.
    for request in [client.head(&room_url), client.get(&room_url), client.get("https://live.douyin.com/")] {
        match request.header("User-Agent", DEFAULT_USER_AGENT).send_limited().await {
            Ok(response) => {
                if matches!(response.status().as_u16(), 401 | 403 | 429) {
                    bail!("抖音平台限制访问，请稍后手动重试");
                }
                let cookies = response.cookies().map(|cookie| {
                    (cookie.name().to_string(), cookie.value().to_string())
                }).collect::<std::collections::BTreeMap<_, _>>();
                if cookies.get("ttwid").is_some_and(|value| !value.is_empty()) {
                    return Ok(cookies.into_iter().map(|(name, value)| format!("{name}={value}"))
                        .collect::<Vec<_>>().join("; "));
                }
                last_error = format!("抖音 Cookie 请求返回 {}，未提供 ttwid", response.status());
            }
            Err(error) => last_error = format!("抖音 Cookie 请求失败: {error}"),
        }
        log::warn!("Diagnostic: room_page.rs:31 (details omitted)");
    }
    bail!("{last_error}，请稍后重试")
}

/// Reads the reflow API for an ephemeral room ID, propagating response errors.
pub(super) async fn reflow_room(client: &Client, room_id: &str, _cookie: &str) -> Result<Value> {
    let response: Value = client.get("https://webcast.amemv.com/webcast/room/reflow/info/")
        .query(&[("type_id", "0"), ("live_id", "1"), ("room_id", room_id),
            ("sec_user_id", ""), ("app_id", "6383")])
        .header("User-Agent", DEFAULT_USER_AGENT)
        .send_limited().await?.error_for_status()?.json().await?;
    response.pointer("/data/room").cloned().context("抖音房间接口未返回房间信息")
}

