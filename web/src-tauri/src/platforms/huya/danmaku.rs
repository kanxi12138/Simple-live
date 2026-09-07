use futures_util::{SinkExt, StreamExt};
use log::info;
use tars_stream::prelude::*;
use tauri::Emitter;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

const WS_URL: &str = "wss://cdnws.api.huya.com";
// 恢复 HEARTBEAT 常量（被误删），供心跳发送使用
const HEARTBEAT: &[u8] = &[0, 20, 29, 0, 12, 44, 54, 0, 76];
// const HEARTBEAT_BASE64: &str = "ABQdAAwsNgBM"; // same as Python
#[allow(dead_code)]
const HEARTBEAT_BASE64: &str = "ABQdAAwsNgBM"; // same as Python

// Minimal JCE/TARS codec for required Huya structures
enum ConnectionOutcome {
    Stop,
    Disconnected,
}

async fn fetch_huya_ids(room_id: &str) -> Result<(i64, i64, i64), String> {
    let client = reqwest::Client::builder().no_proxy().build().map_err(|error| error.to_string())?;
    let page = super::stream_url::fetch_room_page(&client, room_id).await.map_err(|error| error.to_string())?;
    let yyid = page.data.pointer("/roomInfo/tLiveInfo/lYyid").and_then(serde_json::Value::as_i64)
        .ok_or("虎牙页面缺少弹幕用户标识")?;
    if yyid == 0 || page.top_sid == 0 { return Err("虎牙未返回有效的弹幕频道".to_string()); }
    Ok((yyid, page.top_sid, page.sub_sid))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct HuyaJoinParams {
    pub yyid: i64,
    pub top_sid: i64,
    pub sub_sid: i64,
}

#[tauri::command]
pub async fn fetch_huya_join_params(room_id: String) -> Result<HuyaJoinParams, String> {
    match fetch_huya_ids(&room_id).await {
        Ok((ayyuid, top_sid, sub_sid)) => Ok(HuyaJoinParams {
            yyid: ayyuid,
            top_sid,
            sub_sid,
        }),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn start_huya_danmaku_listener(
    payload: crate::platforms::common::GetStreamUrlPayload,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::platforms::common::HuyaDanmakuState>,
) -> Result<(), String> {
    let room_id_or_url = payload.args.room_id_str.clone();
    println!(
        "[Huya Danmaku] start listener room_id_or_url={}",
        room_id_or_url
    );
    info!(
        "[Huya Danmaku] start listener room_id_or_url={}",
        room_id_or_url
    );

    // 停止已有监听
    let previous_tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };
    if let Some(tx) = previous_tx {
        if tx.send(()).await.is_err() {
            eprintln!("[Huya Danmaku] 旧任务关闭失败，可能已退出。");
        }
    }

    // 创建新的关闭通道并保存到 State
    let (tx_shutdown, mut rx_shutdown) = tokio_mpsc::channel::<()>(1);
    {
        let mut lock = state.inner().0.lock().unwrap();
        *lock = Some(tx_shutdown);
    }

    let app_handle_clone = app_handle.clone();
    let room_id_clone = room_id_or_url.clone();

    tokio::spawn(async move {
        println!(
            "[Huya Danmaku] spawned worker for room_id={}",
            room_id_clone
        );
        info!(
            "[Huya Danmaku] spawned worker for room_id={}",
            room_id_clone
        );

        let mut backoff_secs = 1u64;

        loop {
            let result: anyhow::Result<ConnectionOutcome> = async {
                let (ws_url, reg_data) = tokio::select! {
                    _ = rx_shutdown.recv() => return Ok(ConnectionOutcome::Stop),
                    result = get_ws_info_tars(&room_id_clone) => result.map_err(anyhow::Error::msg)?,
                };

                println!(
                    "[Huya Danmaku] ws_url={} reg_len={}",
                    ws_url,
                    reg_data.len()
                );
                info!(
                    "[Huya Danmaku] ws_url={} reg_len={}",
                    ws_url,
                    reg_data.len()
                );

                println!("[Huya Danmaku] connecting to {}", ws_url);
                info!("[Huya Danmaku] connecting to {}", ws_url);
                let (ws_stream, _) = tokio::select! {
                    _ = rx_shutdown.recv() => return Ok(ConnectionOutcome::Stop),
                    result = connect_async(&ws_url) => result?,
                };

                let (mut ws_write, mut ws_read) = ws_stream.split();
                ws_write.send(WsMessage::Binary(reg_data)).await?;

                let hb_task = async {
                    let mut hb_seq = 0usize;
                    while let Ok(_) = ws_write.send(WsMessage::Binary(HEARTBEAT.into())).await {
                        hb_seq += 1;
                        println!("[Huya Danmaku] heartbeat sent #{}", hb_seq);
                        info!("[Huya Danmaku] heartbeat sent #{}", hb_seq);
                        sleep(Duration::from_secs(60)).await;
                    }
                    Err::<(), anyhow::Error>(anyhow::anyhow!("Huya heartbeat send failed"))
                };

                let recv_task = async {
                    while let Some(m) = ws_read.next().await {
                        let m = match m {
                            Ok(x) => x,
                            Err(e) => return Err(anyhow::anyhow!(e)),
                        };
                        match m {
                            WsMessage::Binary(bin) => {
                                let (top_cmd, nested_cmd) = peek_cmds(&bin);
                                println!(
                                    "[Huya Danmaku] WS msg: len={} top_cmd={:?} nested_cmd={:?}",
                                    bin.len(),
                                    top_cmd,
                                    nested_cmd
                                );
                                info!(
                                    "[Huya Danmaku] WS msg: len={} top_cmd={:?} nested_cmd={:?}",
                                    bin.len(),
                                    top_cmd,
                                    nested_cmd
                                );
                                match decode_msg_tars(&bin)? {
                                    Some((nick, text)) => {
                                        println!("[Huya Danmaku] decoded chat: {} -> {}", nick, text);
                                        info!("[Huya Danmaku] decoded chat: {} -> {}", nick, text);
                                        let _ = app_handle_clone.emit(
                                            "danmaku-message",
                                            crate::platforms::common::DanmakuFrontendPayload {
                                                room_id: room_id_clone.clone(),
                                                user: nick,
                                                content: text,
                                                user_level: 0,
                                                fans_club_level: 0,
                                            },
                                        );
                                    }
                                    None => {
                                        if top_cmd == Some(7) {
                                            println!(
                                                "[Huya Danmaku] non-chat or empty msg, nested={:?}",
                                                nested_cmd
                                            );
                                            info!(
                                                "[Huya Danmaku] non-chat or empty msg, nested={:?}",
                                                nested_cmd
                                            );
                                        }
                                    }
                                }
                            }
                            other => {
                                println!("[Huya Danmaku] non-binary ws message: {:?}", other);
                                info!("[Huya Danmaku] non-binary ws message: {:?}", other);
                            }
                        }
                    }
                    anyhow::Ok(())
                };

                tokio::select! {
                    _ = rx_shutdown.recv() => Ok(ConnectionOutcome::Stop),
                    it = hb_task => {
                        if let Err(e) = it { eprintln!("[Huya Danmaku] {}", e); }
                        Ok(ConnectionOutcome::Disconnected)
                    }
                    it = recv_task => {
                        if let Err(e) = it { eprintln!("[Huya Danmaku] recv error: {}", e); }
                        Ok(ConnectionOutcome::Disconnected)
                    }
                }
            }
            .await;

            match result {
                Ok(ConnectionOutcome::Stop) => break,
                Ok(ConnectionOutcome::Disconnected) => {
                    if let Err(emit_error) = app_handle_clone.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id_clone, "platform": "huya", "status": "error", "message": "弹幕连接断开，正在重连"
                    })) { log::error!("Cannot emit danmaku error: {emit_error}"); }
                    eprintln!(
                        "[Huya Danmaku] Disconnected, retrying in {}s.",
                        backoff_secs
                    );
                }
                Err(e) => {
                    if let Err(emit_error) = app_handle_clone.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id_clone, "platform": "huya", "status": "error", "message": e.to_string()
                    })) { log::error!("Cannot emit danmaku error: {emit_error}"); }
                    eprintln!(
                        "[Huya Danmaku] Connection error: {}. Retrying in {}s.",
                        e, backoff_secs
                    );
                }
            }

            let sleep_fut = sleep(Duration::from_secs(backoff_secs));
            tokio::select! {
                _ = sleep_fut => {}
                _ = rx_shutdown.recv() => break,
            }
            backoff_secs = (backoff_secs * 2).min(30);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_huya_danmaku_listener(
    room_id: String,
    state: tauri::State<'_, crate::platforms::common::HuyaDanmakuState>,
) -> Result<(), String> {
    println!(
        "[Huya Danmaku] stop_huya_danmaku_listener called for room_id={}",
        room_id
    );

    // 取出当前监听的停止信号发送器
    let tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };

    if let Some(tx) = tx {
        if let Err(_) = tx.send(()).await {
            println!("[Huya Danmaku] 停止信号发送失败，监听器可能已经退出");
        } else {
            println!("[Huya Danmaku] 停止信号已发送给 room_id={}", room_id);
        }
    } else {
        println!("[Huya Danmaku] 没有找到活跃的监听器需要停止");
    }

    Ok(())
}

// 采用 tars_stream 的实现（参考 all_in_one.rs），保留 Tauri 命令，对旧 jce 逻辑停用

struct HuyaUser {
    _uid: i64,
    _imid: i64,
    name: String,
    _gender: i32,
}

struct HuyaDanmakuFmt {
    color: i32,
}

impl StructFromTars for HuyaUser {
    fn _decode_from(decoder: &mut TarsDecoder) -> Result<Self, DecodeErr> {
        let uid = decoder.read_int64(0, false, -1)?;
        let imid = decoder.read_int64(1, false, -1)?;
        let name = decoder.read_string(2, false, "".to_string())?;
        let gender = decoder.read_int32(3, false, -1)?;
        Ok(HuyaUser {
            _uid: uid,
            _imid: imid,
            name,
            _gender: gender,
        })
    }
}

impl StructFromTars for HuyaDanmakuFmt {
    fn _decode_from(decoder: &mut TarsDecoder) -> Result<Self, DecodeErr> {
        let color = decoder.read_int32(0, false, 16777215)?;
        Ok(HuyaDanmakuFmt { color })
    }
}

fn peek_cmds(data: &[u8]) -> (Option<i32>, Option<i64>) {
    let mut ios = TarsDecoder::from(data);
    let top_cmd = ios.read_int32(0, false, -1).ok();
    let nested_cmd = ios
        .read_bytes(1, false, Default::default())
        .ok()
        .and_then(|b1| {
            let mut inner = TarsDecoder::from(b1.as_ref());
            inner.read_int32(1, false, -1).ok().map(|v| v as i64)
        });
    (top_cmd, nested_cmd)
}

// Ported from dart_simple_live ba828e6 huya_danmaku.dart (GPL-3.0).
async fn get_ws_info_tars(room_id: &str) -> Result<(String, Vec<u8>), String> {
    let (yyid, top_sid, sub_sid) = fetch_huya_ids(room_id).await?;
    let encode = || -> Result<Vec<u8>, EncodeErr> {
        let mut registration = TarsEncoder::new();
        registration.write_int64(0, yyid)?;
        registration.write_boolean(1, true)?;
        registration.write_string(2, &String::new())?;
        registration.write_string(3, &String::new())?;
        registration.write_int64(4, top_sid)?;
        registration.write_int64(5, sub_sid)?;
        registration.write_int32(6, 0)?;
        registration.write_int32(7, 0)?;
        let mut command = TarsEncoder::new();
        command.write_int32(0, 1)?;
        command.write_bytes(1, &registration.to_bytes())?;
        Ok(command.to_bytes().to_vec())
    };
    Ok((WS_URL.to_string(), encode().map_err(|error| error.to_string())?))
}

fn decode_msg_tars(data: &[u8]) -> anyhow::Result<Option<(String, String)>> {
    let mut ret: Option<(String, String)> = None;
    let mut ios = TarsDecoder::from(data);
    let top = ios.read_int32(0, false, -1)?;
    if top != 7 {
        println!("[Huya Danmaku] ignore msg: top_cmd={}", top);
        info!("[Huya Danmaku] ignore msg: top_cmd={}", top);
        return Ok(ret);
    }
    let b1 = ios.read_bytes(1, false, Default::default())?;
    let mut inner = TarsDecoder::from(b1.as_ref());
    let nested = inner.read_int32(1, false, -1).unwrap_or(-1);
    let b2 = inner.read_bytes(2, false, Default::default())?;
    println!("[Huya Danmaku] nested={} payload_len={}", nested, b2.len());
    info!("[Huya Danmaku] nested={} payload_len={}", nested, b2.len());
    let mut payload = TarsDecoder::from(b2.as_ref());

    if nested == 1400 {
        let user = payload
            .read_struct(
                0,
                false,
                HuyaUser {
                    _uid: -1,
                    _imid: -1,
                    name: "".to_owned(),
                    _gender: 1,
                },
            )
            .unwrap_or(HuyaUser {
                _uid: -1,
                _imid: -1,
                name: "".to_owned(),
                _gender: 1,
            });
        let text = payload
            .read_string(3, false, "".to_owned())
            .unwrap_or_default();
        let fmt = payload
            .read_struct(6, false, HuyaDanmakuFmt { color: 16777215 })
            .unwrap_or(HuyaDanmakuFmt { color: 16777215 });
        if !text.is_empty() {
            let nick = if !user.name.is_empty() {
                user.name
            } else {
                "匿名".to_string()
            };
            let _color_hex = format!("{:06x}", if fmt.color <= 0 { 16777215 } else { fmt.color });
            println!(
                "[Huya Danmaku] decoded nested=1400 nick={} text={}",
                nick, text
            );
            info!(
                "[Huya Danmaku] decoded nested=1400 nick={} text={}",
                nick, text
            );
            ret = Some((nick, text));
        } else {
            println!("[Huya Danmaku] empty text in nested=1400");
            info!("[Huya Danmaku] empty text in nested=1400");
        }
    } else {
        println!("[Huya Danmaku] non-chat nested={}, skip", nested);
        info!("[Huya Danmaku] non-chat nested={}, skip", nested);
    }
    Ok(ret)
}

