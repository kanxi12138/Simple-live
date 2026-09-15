use crate::platforms::common::request_limit::LimitedRequest;
// Ported from dart_simple_live ba828e6, bilibili_site.dart (GPL-3.0).
// Copyright xiaoyaocz and contributors; see THIRD_PARTY_NOTICES.md.
use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT};
use serde::Serialize;
use serde_json::Value;

use crate::platforms::common::{GetStreamUrlPayload, LiveStreamInfo};
use crate::platforms::common::types::StreamVariant;

const PLAY_ENDPOINT: &str = "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo";
const PLAY_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";
pub(super) const BILIBILI_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36 Edg/115.0.1901.188";

/// Existing stream response extended with platform-provided quality names and headers.
#[derive(Serialize)]
pub struct BilibiliPlaybackResponse {
    #[serde(flatten)]
    pub info: LiveStreamInfo,
    pub qualities: Vec<String>,
    pub headers: BTreeMap<String, String>,
}

/// Builds runtime cookie/buvid headers; invalid cookies and fingerprint failures propagate.
pub(super) async fn runtime_client(cookie: Option<&str>) -> Result<(reqwest::Client, String)> {
    let bootstrap = reqwest::Client::builder().redirect(crate::network_policy::redirects()).no_proxy().build()?;
    let mut cookie = cookie.unwrap_or_default().trim().to_string();
    super::search::ensure_buvid(&bootstrap, &mut cookie).await.map_err(anyhow::Error::msg)?;
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(BILIBILI_USER_AGENT));
    headers.insert(REFERER, HeaderValue::from_static("https://live.bilibili.com/"));
    headers.insert(COOKIE, HeaderValue::from_str(&cookie)?);
    Ok((reqwest::Client::builder().redirect(crate::network_policy::redirects()).no_proxy().default_headers(headers).build()?, cookie))
}

/// Signs query parameters using keys acquired from the current Bilibili session.
pub(super) async fn signed_query(client: &reqwest::Client, parameters: Vec<(&str, String)>) -> Result<String> {
    let navigation: Value = client.get("https://api.bilibili.com/x/web-interface/nav")
        .send_limited().await?.error_for_status()?.json().await?;
    let key = |name: &str| -> Result<String> {
        let link = navigation["data"]["wbi_img"][name].as_str()
            .with_context(|| format!("B站 nav 未返回 WBI 密钥（code={}）", navigation["code"]))?;
        let filename = link.rsplit('/').next().and_then(|part| part.split('.').next())
            .context("B站 WBI 密钥格式错误")?;
        if filename.len() != 32 || !filename.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("B站 WBI 密钥无效");
        }
        Ok(filename.to_string())
    };
    Ok(super::auth::encode_wbi(parameters, (key("img_url")?, key("sub_url")?)))
}

/// Resolves short room IDs and live status without requesting signed room metadata.
pub(super) async fn room_init(client: &reqwest::Client, room_id: &str) -> Result<Value> {
    let room_id = room_id.trim();
    if room_id.is_empty() || !room_id.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("B站房间号必须是数字");
    }
    let response = client.get("https://api.live.bilibili.com/room/v1/Room/room_init")
        .query(&[("id", room_id)]).send_limited().await.context("B站 room_init 网络请求失败")?
        .error_for_status().context("B站 room_init HTTP 请求失败")?.json().await.context("B站 room_init JSON 无效")?;
    response_data(response, "room_init")
}

fn response_data(response: Value, stage: &str) -> Result<Value> {
    if response["code"].as_i64() != Some(0) {
        bail!("B站 {stage} 请求失败（code={}）: {}", response["code"], response["message"].as_str().or_else(|| response["msg"].as_str()).unwrap_or("平台限制或响应异常"));
    }
    response.get("data").filter(|data| !data.is_null()).cloned().with_context(|| format!("B站 {stage} 响应缺少数据"))
}

async fn play_info(client: &reqwest::Client, room_id: &str, quality: Option<i32>) -> Result<Value> {
    let mut parameters = vec![
        ("room_id", room_id.to_string()), ("protocol", "0,1".to_string()),
        ("format", "0,1,2".to_string()),
        ("codec", "0".to_string()),
        ("platform", "html5".to_string()), ("dolby", "5".to_string()),
    ];
    if let Some(quality) = quality { parameters.push(("qn", quality.to_string())); }
    let response = client.get(PLAY_ENDPOINT).query(&parameters).send_limited().await.context("B站 getRoomPlayInfo 网络请求失败")?
        .error_for_status().context("B站 getRoomPlayInfo HTTP 请求失败")?.json().await.context("B站 getRoomPlayInfo JSON 无效")?;
    response_data(response, "getRoomPlayInfo")?.pointer("/playurl_info/playurl").cloned().context("B站 getRoomPlayInfo 未返回播放信息")
}

fn collect_variants(play: &Value, descriptions: &BTreeMap<i32, String>) -> Result<Vec<StreamVariant>> {
    let streams = play["stream"].as_array().context("B站未返回播放线路")?;
    let mut variants = Vec::new();
    for stream in streams {
        for format in stream["format"].as_array().context("B站播放格式缺失")? {
            for codec in format["codec"].as_array().context("B站编码信息缺失")? {
                let base = codec["base_url"].as_str().context("B站流路径缺失")?;
                let qn = codec["current_qn"].as_i64().context("B站实际画质缺失")? as i32;
                for line in codec["url_info"].as_array().context("B站 CDN 信息缺失")? {
                    let host = line["host"].as_str().context("B站 CDN 地址缺失")?;
                    let extra = line["extra"].as_str().context("B站流参数缺失")?;
                    variants.push(StreamVariant {
                        url: format!("{host}{base}{extra}"),
                        format: format["format_name"].as_str().map(str::to_string),
                        protocol: stream["protocol_name"].as_str().map(str::to_string),
                        qn: Some(qn), desc: descriptions.get(&qn).cloned(),
                    });
                }
            }
        }
    }
    Ok(variants)
}

// Adapted from chen-zeong/DTV stream_url.rs (MIT); see THIRD_PARTY_NOTICES.md.
async fn probe_hls(client: &reqwest::Client, variants: &[StreamVariant], preferred: bool) -> Option<usize> {
    for (index, variant) in variants.iter().enumerate().filter(|(_, variant)| {
        (variant.protocol.as_deref().is_some_and(|protocol| protocol.contains("hls"))
            || matches!(variant.format.as_deref(), Some("ts" | "fmp4" | "mp4" | "m4s" | "m3u8")))
            && variant.url.contains("d1--cn") == preferred
    }).take(4) {
        let Ok(url) = reqwest::Url::parse(&variant.url) else { continue; };
        if crate::network_policy::stream_platform(&url) != Some("bilibili") { continue; }
        // A failed CDN probe is recoverable: try the next returned line.
        if let Ok(response) = client.get(url).header(REFERER, "https://live.bilibili.com/")
            .header(reqwest::header::ORIGIN, "https://live.bilibili.com").send_limited().await {
            if response.status().is_success() { return Some(index); }
        }
    }
    None
}

async fn resolve_stream(payload: GetStreamUrlPayload, quality: String, cookie: Option<String>) -> Result<BilibiliPlaybackResponse> {
    let room_id = payload.args.room_id_str.trim();
    if room_id.is_empty() || !room_id.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("B站房间号必须是数字");
    }
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(PLAY_USER_AGENT));
    headers.insert(REFERER, HeaderValue::from_static("https://live.bilibili.com/"));
    headers.insert(reqwest::header::ORIGIN, HeaderValue::from_static("https://live.bilibili.com"));
    if let Some(cookie) = cookie.as_deref().map(str::trim).filter(|cookie| !cookie.is_empty()) {
        headers.insert(COOKIE, HeaderValue::from_str(cookie).context("B站播放 Cookie 格式无效")?);
    }
    let client = reqwest::Client::builder().redirect(crate::network_policy::redirects()).no_proxy()
        .default_headers(headers).http1_only().connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(15)).build()?;
    // CDN probes must not receive the API client's login Cookie.
    let probe_client = reqwest::Client::builder().redirect(crate::network_policy::redirects()).no_proxy()
        .user_agent(PLAY_USER_AGENT).http1_only().timeout(std::time::Duration::from_secs(15)).build()?;
    let detail = room_init(&client, room_id).await?;
    let real_id = detail["room_id"].as_u64().context("B站 room_init 真实房间号缺失")?.to_string();
    let status = detail["live_status"].as_i64().context("B站 room_init 直播状态缺失")? as i32;
    let mut response = BilibiliPlaybackResponse {
        info: LiveStreamInfo {
            title: detail["title"].as_str().map(str::to_string),
            anchor_name: detail["uname"].as_str().map(str::to_string),
            avatar: None,
            stream_url: None, status: Some(status), error_message: None, upstream_url: None,
            available_streams: None, normalized_room_id: Some(real_id.clone()), web_rid: None,
        },
        qualities: Vec::new(),
        headers: BTreeMap::from([
            ("Referer".to_string(), "https://live.bilibili.com".to_string()),
            ("User-Agent".to_string(), PLAY_USER_AGENT.to_string()),
            ("Origin".to_string(), "https://live.bilibili.com".to_string()),
        ]),
    };
    if status != 1 { return Ok(response); }
    let play = play_info(&client, &real_id, None).await?;
    let descriptions: BTreeMap<i32, String> = play["g_qn_desc"].as_array().context("B站画质描述缺失")?
        .iter().filter_map(|item| Some((item["qn"].as_i64()? as i32, item["desc"].as_str()?.to_string()))).collect();
    let accepted: Vec<i32> = play.pointer("/stream/0/format/0/codec/0/accept_qn")
        .and_then(Value::as_array).context("B站可选画质缺失")?
        .iter().filter_map(|value| value.as_i64().map(|quality| quality as i32)).collect();
    response.qualities = accepted.iter().filter_map(|quality| descriptions.get(quality).cloned()).collect();
    let selected = accepted.iter().find(|candidate| descriptions.get(candidate) == Some(&quality))
        .or_else(|| accepted.iter().max()).context("B站没有可用画质")?;
    let mut chosen = None;
    let mut fallback = None;
    for _ in 0..=3 {
        let selected_play = play_info(&client, &real_id, Some(*selected)).await?;
        let mut variants = collect_variants(&selected_play, &descriptions)?;
        let selected_index = variants.iter().position(|variant| variant.format.as_deref() == Some("flv"));
        let selected_index = match selected_index {
            Some(index) => Some(index),
            None => probe_hls(&probe_client, &variants, true).await,
        };
        if let Some(index) = selected_index {
            // The frontend defaults to the first variant; preserve the chosen line.
            let selected_variant = variants.remove(index);
            variants.insert(0, selected_variant);
            chosen = Some(variants);
            break;
        }
        if fallback.is_none() {
            if let Some(index) = probe_hls(&probe_client, &variants, false).await {
                let selected_variant = variants.remove(index);
                variants.insert(0, selected_variant);
                fallback = Some(variants);
            }
        }
    }
    let variants = chosen.or(fallback).context("B站 getRoomPlayInfo 未找到可用的 FLV/HLS 直播线路")?;
    response.info.stream_url = variants.first().map(|variant| variant.url.clone());
    response.info.upstream_url = response.info.stream_url.clone();
    response.info.available_streams = Some(variants);
    Ok(response)
}

/// Resolves `payload` with the requested quality and optional user cookie.
/// Returns existing response fields plus actual options; HTTP and protocol failures are errors.
#[tauri::command]
pub async fn get_bilibili_live_stream_url_with_quality(
    payload: GetStreamUrlPayload,
    quality: String,
    cookie: Option<String>,
) -> Result<BilibiliPlaybackResponse, String> {
    resolve_stream(payload, quality, cookie).await.map_err(|error| format!("{error:#}"))
}
