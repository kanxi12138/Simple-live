use crate::platforms::common::http_client::HttpClient;
use crate::platforms::common::types::StreamVariant;
use crate::platforms::common::GetStreamUrlPayload;
use crate::platforms::common::LiveStreamInfo as CommonLiveStreamInfo;
use crate::platforms::douyin::web_api::{
    fetch_room_data, normalize_douyin_live_id, DouyinRoomData,
};
use crate::proxy::ProxyServerHandle;
use crate::StreamUrlStore;
use serde_json::Value;
use tauri::{command, AppHandle, State};

const QUALITY_OD: &str = "OD";
#[command]
pub async fn get_douyin_live_stream_url(
    app_handle: AppHandle,
    stream_url_store: State<'_, StreamUrlStore>,
    proxy_server_handle: State<'_, ProxyServerHandle>,
    payload: GetStreamUrlPayload,
) -> Result<CommonLiveStreamInfo, String> {
    get_douyin_live_stream_url_with_quality(
        app_handle,
        stream_url_store,
        proxy_server_handle,
        payload,
        QUALITY_OD.to_string(),
    )
    .await
}

#[command]
pub async fn get_douyin_live_stream_url_with_quality(
    _app_handle: AppHandle,
    _stream_url_store: State<'_, StreamUrlStore>,
    _proxy_server_handle: State<'_, ProxyServerHandle>,
    payload: GetStreamUrlPayload,
    quality: String,
) -> Result<CommonLiveStreamInfo, String> {
    let requested_id = payload.args.room_id_str.trim().to_string();
    if requested_id.is_empty() {
        return Ok(CommonLiveStreamInfo {
            title: None,
            anchor_name: None,
            avatar: None,
            stream_url: None,
            status: None,
            error_message: Some("Douyin web_id cannot be empty.".to_string()),
            upstream_url: None,
            available_streams: None,
            normalized_room_id: None,
            web_rid: None,
        });
    }

    println!("Diagnostic: douyin_streamer_detail.rs:55 (details omitted)");

    let http_client = HttpClient::new_direct_connection()
        .map_err(|e| format!("Failed to create direct connection HttpClient: {}", e))?;

    let normalized_id = normalize_douyin_live_id(&requested_id);
    let DouyinRoomData { room } = fetch_room_data(&http_client, &normalized_id, None).await?;
    let web_rid = extract_web_rid(&room).unwrap_or_else(|| normalized_id.clone());
    let status = room
        .get("status")
        .and_then(|v| v.as_i64())
        .unwrap_or_default() as i32;
    let title = room
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let anchor_name = extract_anchor_name(&room);
    let avatar = extract_avatar(&room);
    let available_streams = collect_available_streams(&room);

    if status != 2 {
        println!("Diagnostic: douyin_streamer_detail.rs:79 (details omitted)");
        return Ok(CommonLiveStreamInfo {
            title,
            anchor_name,
            avatar,
            stream_url: None,
            status: Some(status),
            error_message: None,
            upstream_url: None,
            available_streams: available_streams.clone(),
            normalized_room_id: None,
            web_rid: Some(web_rid),
        });
    }

    let streams = available_streams.as_ref().ok_or("抖音未返回可用画质和线路")?;
    let selected = streams.iter().find(|stream| stream.desc.as_deref() == Some(quality.as_str()))
        .or_else(|| streams.first()).ok_or("抖音未返回播放地址")?;
    let sanitized_url = selected.url.clone();

    Ok(CommonLiveStreamInfo {
        title,
        anchor_name,
        avatar,
        stream_url: Some(sanitized_url.clone()),
        status: Some(status),
        error_message: None,
        upstream_url: Some(sanitized_url),
        available_streams,
        normalized_room_id: None,
        web_rid: Some(web_rid),
    })
}

pub(crate) fn extract_web_rid(room: &Value) -> Option<String> {
    room.get("owner")
        .and_then(|o| o.get("web_rid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            room.get("anchor")
                .and_then(|a| a.get("web_rid"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            room.get("web_rid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

pub(crate) fn extract_anchor_name(room: &Value) -> Option<String> {
    room.get("anchor_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            room.get("owner")
                .and_then(|o| o.get("nickname"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            room.get("anchor")
                .and_then(|a| a.get("nickname"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

pub(crate) fn extract_avatar(room: &Value) -> Option<String> {
    room.get("owner")
        .and_then(|o| o.get("avatar_thumb"))
        .and_then(|thumb| thumb.get("url_list"))
        .and_then(|list| list.get(0))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            room.get("anchor")
                .and_then(|a| a.get("avatar_thumb"))
                .and_then(|thumb| thumb.get("url_list"))
                .and_then(|list| list.get(0))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

// dart_simple_live ba828e6: preserve SDK quality labels and sort by descending level.
pub(crate) fn collect_available_streams(room: &Value) -> Option<Vec<StreamVariant>> {
    let stream = room.get("stream_url")?;
    let pull = stream.pointer("/live_core_sdk_data/pull_data")?;
    let mut qualities = pull.pointer("/options/qualities")?.as_array()?.clone();
    qualities.sort_by_key(|quality| std::cmp::Reverse(quality["level"].as_i64().unwrap_or(0)));
    let data = pull["stream_data"].as_str().and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let mut variants = Vec::new();
    for quality in qualities {
        let Some(name) = quality["name"].as_str() else { continue; };
        let key = quality["sdk_key"].as_str().unwrap_or_default();
        for (format, map_name) in [("flv", "flv_pull_url"), ("hls", "hls_pull_url_map")] {
            let url = if let Some(data) = data.as_ref() {
                data["data"][key]["main"][format].as_str()
            } else {
                let urls = stream[map_name].as_object();
                let level = quality["level"].as_u64().unwrap_or(0) as usize;
                urls.and_then(|urls| urls.len().checked_sub(level)
                    .and_then(|index| urls.values().nth(index)).and_then(Value::as_str))
            };
            if let Some(url) = url.filter(|url| !url.is_empty()) {
                variants.push(StreamVariant {
                    url: url.to_string(), format: Some(format.to_string()),
                    desc: Some(name.to_string()), qn: None, protocol: Some(format.to_string()),
                });
            }
        }
    }
    if variants.is_empty() { None } else { Some(variants) }
}
