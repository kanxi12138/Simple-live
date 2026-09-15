use crate::platforms::common::request_limit::LimitedRequest;
// Ported from dart_simple_live ba828e6 bilibili_danmaku.dart (GPL-3.0).
// Copyright xiaoyaocz and contributors; see THIRD_PARTY_NOTICES.md.
use std::io::Read;
use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tauri::Emitter;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const HEADER_LENGTH: usize = 16;
const AUTH: u32 = 7;
const AUTH_REPLY: u32 = 8;
const HEARTBEAT: u32 = 2;
const CHAT: u32 = 5;
const MAX_PACKET: u64 = 16 * 1024 * 1024;

fn packet(operation: u32, body: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(HEADER_LENGTH + body.len());
    packet.extend_from_slice(&((HEADER_LENGTH + body.len()) as u32).to_be_bytes());
    packet.extend_from_slice(&(HEADER_LENGTH as u16).to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());
    packet.extend_from_slice(&operation.to_be_bytes());
    packet.extend_from_slice(&1u32.to_be_bytes());
    packet.extend_from_slice(body);
    packet
}

fn cookie_value<'a>(cookie: &'a str, name: &str) -> Option<&'a str> {
    cookie.split(';').filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then_some(value))
}

fn process_packets(data: &[u8], app: &tauri::AppHandle, room: &str) -> Result<()> {
    let mut pending = vec![data.to_vec()];
    let mut decoded_bytes = data.len() as u64;
    while let Some(data) = pending.pop() {
        let mut offset = 0;
        while offset < data.len() {
            let header = data.get(offset..offset + HEADER_LENGTH).context("B站弹幕包头不完整")?;
            let length = u32::from_be_bytes(header[0..4].try_into()?) as usize;
            let header_length = u16::from_be_bytes(header[4..6].try_into()?) as usize;
            let version = u16::from_be_bytes(header[6..8].try_into()?);
            let operation = u32::from_be_bytes(header[8..12].try_into()?);
            if header_length < HEADER_LENGTH || length < header_length || length > data.len() - offset {
                bail!("B站弹幕包长度无效");
            }
            let body = &data[offset + header_length..offset + length];
            offset += length;
            if version == 2 || version == 3 {
                let decoder: Box<dyn Read + '_> = if version == 2 {
                    Box::new(flate2::read::ZlibDecoder::new(body))
                } else { Box::new(brotlic::DecompressorReader::new(body)) };
                let mut decoded = Vec::new();
                decoder.take(MAX_PACKET + 1).read_to_end(&mut decoded)?;
                decoded_bytes += decoded.len() as u64;
                if decoded_bytes > MAX_PACKET { bail!("B站弹幕解压数据过大"); }
                pending.push(decoded);
            } else if operation == AUTH_REPLY {
                let response: Value = serde_json::from_slice(body)?;
                if response["code"].as_i64() != Some(0) { bail!("B站弹幕认证失败: {}", response["code"]); }
                app.emit("danmaku-status", json!({"room_id": room, "platform": "bilibili", "status": "ready"}))?;
            } else if operation == CHAT {
                emit_chat(body, app, room)?;
            }
        }
    }
    Ok(())
}

fn emit_chat(body: &[u8], app: &tauri::AppHandle, room: &str) -> Result<()> {
    let message: Value = serde_json::from_slice(body)?;
    if !message["cmd"].as_str().unwrap_or_default().starts_with("DANMU_MSG") { return Ok(()); }
    let info = &message["info"];
    app.emit("danmaku-message", crate::platforms::common::DanmakuFrontendPayload {
        room_id: room.to_string(), user: info[2][1].as_str().unwrap_or_default().to_string(),
        content: info[1].as_str().context("B站聊天消息缺少内容")?.to_string(),
        user_level: info[4][0].as_i64().unwrap_or(0),
        fans_club_level: info[3][0].as_i64().unwrap_or(0) as i32,
    })?;
    Ok(())
}

async fn run_listener(room: &str, cookie: Option<&str>, app: &tauri::AppHandle) -> Result<()> {
    let (client, cookie) = super::stream_url::runtime_client(cookie).await?;
    let room_info = super::stream_url::room_info(&client, room).await?;
    let real_room = room_info["room_info"]["room_id"].as_u64().context("B站未返回真实房间号")?;
    let query = super::stream_url::signed_query(&client, vec![("id", real_room.to_string()), ("type", "0".to_string())]).await?;
    let response: Value = client.get(format!("https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?{query}"))
        .send_limited().await?.error_for_status()?.json().await?;
    if response["code"].as_i64() != Some(0) { bail!("B站弹幕服务器请求失败: {}", response["message"]); }
    let token = response["data"]["token"].as_str().filter(|token| !token.is_empty()).context("B站未返回弹幕 Token")?;
    let hosts = response["data"]["host_list"].as_array().context("B站未返回弹幕服务器")?;
    let mut connection = None;
    for host in hosts.iter().take(3) {
        let hostname = host["host"].as_str().context("B站弹幕主机无效")?;
        let port = host["wss_port"].as_u64().context("B站弹幕端口缺失")?;
        let target = reqwest::Url::parse(&format!("https://{hostname}:{port}/sub"))?;
        if !crate::network_policy::has_domain(&target, &["bilibili.com"]) {
            bail!("B站弹幕目标不在允许域名内");
        }
        match connect_async(format!("wss://{hostname}:{port}/sub")).await {
            Ok((socket, _)) => { connection = Some(socket); break; }
            Err(_) => log::warn!("Diagnostic: danmaku.rs:99 (details omitted)"),
        }
    }
    let mut socket = connection.context("B站弹幕服务器均连接失败")?;
    let auth = json!({"uid": cookie_value(&cookie, "DedeUserID").and_then(|value| value.parse::<u64>().ok()).unwrap_or(0),
        "roomid": real_room, "protover": 3, "platform": "web", "type": 2,
        "buvid": cookie_value(&cookie, "buvid3").context("B站缺少 buvid3")?, "key": token});
    socket.send(Message::Binary(packet(AUTH, &serde_json::to_vec(&auth)?))).await?;
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = heartbeat.tick() => socket.send(Message::Binary(packet(HEARTBEAT, &[]))).await?,
            message = socket.next() => match message.context("B站弹幕连接已关闭")?? {
                Message::Binary(data) => process_packets(&data, app, room)?,
                Message::Ping(body) => socket.send(Message::Pong(body)).await?,
                Message::Close(_) => bail!("B站弹幕连接已关闭"),
                _ => {},
            }
        }
    }
}

/// Starts one cancellable Bilibili session; protocol failures are emitted to the UI.
#[tauri::command]
pub async fn start_bilibili_danmaku_listener(
    payload: crate::platforms::common::GetStreamUrlPayload,
    cookie: Option<String>, app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::platforms::common::BilibiliDanmakuState>,
) -> Result<(), String> {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let previous = state.0.lock().map_err(|error| error.to_string())?.replace(sender);
    if let Some(previous) = previous { let _ = previous.send(()).await; }
    tauri::async_runtime::spawn(async move {
        let room = payload.args.room_id_str;
        tokio::select! {
            _ = receiver.recv() => {},
            result = run_listener(&room, cookie.as_deref(), &app_handle) => {
                if let Err(error) = result {
                    log::error!("Diagnostic: danmaku.rs:137 (details omitted)");
                    if let Err(_emit_error) = app_handle.emit("danmaku-status", json!({"room_id": room,
                        "platform": "bilibili", "status": "error", "message": error.to_string()})) {
                        log::error!("Diagnostic: danmaku.rs:140 (details omitted)");
                    }
                }
            }
        }
    });
    Ok(())
}

/// Cancels setup, socket reads and heartbeat without waiting for incoming traffic.
#[tauri::command]
pub async fn stop_bilibili_danmaku_listener(
    state: tauri::State<'_, crate::platforms::common::BilibiliDanmakuState>,
) -> Result<(), String> {
    let previous = state.0.lock().map_err(|error| error.to_string())?.take();
    if let Some(previous) = previous { let _ = previous.send(()).await; }
    Ok(())
}
