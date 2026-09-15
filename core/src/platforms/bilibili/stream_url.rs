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
        let link = navigation["data"]["wbi_img"][name].as_str().context("B站未返回 WBI 密钥")?;
        let filename = link.rsplit('/').next().and_then(|part| part.split('.').next())
            .context("B站 WBI 密钥格式错误")?;
        if filename.len() != 32 || !filename.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("B站 WBI 密钥无效");
        }
        Ok(filename.to_string())
    };
    Ok(super::auth::encode_wbi(parameters, (key("img_url")?, key("sub_url")?)))
}

/// Resolves a short room ID and metadata using the signed room information endpoint.
pub(super) async fn room_info(client: &reqwest::Client, room_id: &str) -> Result<Value> {
    if room_id.is_empty() || !room_id.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("B站房间号必须是数字");
    }
    let query = signed_query(client, vec![("room_id", room_id.to_string())]).await?;
    let response: Value = client.get(format!("https://api.live.bilibili.com/xlive/web-room/v1/index/getInfoByRoom?{query}"))
        .send_limited().await?.error_for_status()?.json().await?;
    response_data(response)
}

fn response_data(response: Value) -> Result<Value> {
    if response["code"].as_i64() != Some(0) {
        bail!("B站请求失败: {}", response["message"].as_str().unwrap_or("平台限制或响应异常"));
    }
    response.get("data").cloned().context("B站响应缺少数据")
}

async fn play_info(client: &reqwest::Client, room_id: &str, quality: Option<i32>) -> Result<Value> {
    let mut parameters = vec![
        ("room_id", room_id.to_string()), ("protocol", "0,1".to_string()),
        ("format", if quality.is_some() { "0,2" } else { "0,1,2" }.to_string()),
        ("codec", if quality.is_some() { "0" } else { "0,1" }.to_string()),
        ("platform", "web".to_string()),
    ];
    if let Some(quality) = quality { parameters.push(("qn", quality.to_string())); }
    let response = client.get(PLAY_ENDPOINT).query(&parameters).send_limited().await?
        .error_for_status()?.json().await?;
    response_data(response)?.pointer("/playurl_info/playurl").cloned().context("B站未返回播放信息")
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
    variants.sort_by_key(|variant| variant.url.contains("mcdn"));
    if variants.is_empty() { bail!("B站未返回可用播放线路"); }
    Ok(variants)
}

async fn resolve_stream(payload: GetStreamUrlPayload, quality: String, cookie: Option<String>) -> Result<BilibiliPlaybackResponse> {
    let (client, _) = runtime_client(cookie.as_deref()).await?;
    let detail = room_info(&client, payload.args.room_id_str.trim()).await?;
    let real_id = detail["room_info"]["room_id"].as_u64().context("B站真实房间号缺失")?.to_string();
    let status = detail["room_info"]["live_status"].as_i64().context("B站直播状态缺失")? as i32;
    let mut response = BilibiliPlaybackResponse {
        info: LiveStreamInfo {
            title: detail["room_info"]["title"].as_str().map(str::to_string),
            anchor_name: detail["anchor_info"]["base_info"]["uname"].as_str().map(str::to_string),
            avatar: detail["anchor_info"]["base_info"]["face"].as_str().map(str::to_string),
            stream_url: None, status: Some(status), error_message: None, upstream_url: None,
            available_streams: None, normalized_room_id: Some(real_id.clone()), web_rid: None,
        },
        qualities: Vec::new(),
        headers: BTreeMap::from([
            ("Referer".to_string(), "https://live.bilibili.com".to_string()),
            ("User-Agent".to_string(), BILIBILI_USER_AGENT.to_string()),
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
        .or_else(|| accepted.first()).context("B站没有可用画质")?;
    let selected_play = play_info(&client, &real_id, Some(*selected)).await?;
    let variants = collect_variants(&selected_play, &descriptions)?;
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
    resolve_stream(payload, quality, cookie).await.map_err(|error| error.to_string())
}
