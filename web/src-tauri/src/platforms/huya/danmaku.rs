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
    let client = reqwest::Client::builder().redirect(crate::network_policy::redirects()).no_proxy().build().map_err(|error| error.to_string())?;
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
    println!("Diagnostic: danmaku.rs:57 (details omitted)");
    info!("Diagnostic: danmaku.rs:61 (details omitted)");

    // 停止已有监听
    let previous_tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };
    if let Some(tx) = previous_tx {
        if tx.send(()).await.is_err() {
            eprintln!("Diagnostic: danmaku.rs:73 (details omitted)");
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
        println!("Diagnostic: danmaku.rs:88 (details omitted)");
        info!("Diagnostic: danmaku.rs:92 (details omitted)");

        let mut backoff_secs = 1u64;

        for _attempt in 0..3 {
            let result: anyhow::Result<ConnectionOutcome> = async {
                let (ws_url, reg_data) = tokio::select! {
                    _ = rx_shutdown.recv() => return Ok(ConnectionOutcome::Stop),
                    result = get_ws_info_tars(&room_id_clone) => result.map_err(anyhow::Error::msg)?,
                };

                println!("Diagnostic: danmaku.rs:106 (details omitted)");
                info!("Diagnostic: danmaku.rs:111 (details omitted)");

                println!("Diagnostic: danmaku.rs:117 (details omitted)");
                info!("Diagnostic: danmaku.rs:118 (details omitted)");
                let (ws_stream, _) = tokio::select! {
                    _ = rx_shutdown.recv() => return Ok(ConnectionOutcome::Stop),
                    result = connect_async(&ws_url) => result?,
                };

                let (mut ws_write, mut ws_read) = ws_stream.split();
                ws_write.send(WsMessage::Binary(reg_data)).await?;

                let hb_task = async {

                    while let Ok(_) = ws_write.send(WsMessage::Binary(HEARTBEAT.into())).await {

                        println!("Diagnostic: danmaku.rs:131 (details omitted)");
                        info!("Diagnostic: danmaku.rs:132 (details omitted)");
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
                                let (top_cmd, _nested_cmd) = peek_cmds(&bin);
                                println!("Diagnostic: danmaku.rs:147 (details omitted)");
                                info!("Diagnostic: danmaku.rs:153 (details omitted)");
                                match decode_msg_tars(&bin)? {
                                    Some((nick, text)) => {
                                        println!("Diagnostic: danmaku.rs:161 (details omitted)");
                                        info!("Diagnostic: danmaku.rs:162 (details omitted)");
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
                                            println!("Diagnostic: danmaku.rs:176 (details omitted)");
                                            info!("Diagnostic: danmaku.rs:180 (details omitted)");
                                        }
                                    }
                                }
                            }
                            _other => {
                                println!("Diagnostic: danmaku.rs:189 (details omitted)");
                                info!("Diagnostic: danmaku.rs:190 (details omitted)");
                            }
                        }
                    }
                    anyhow::Ok(())
                };

                tokio::select! {
                    _ = rx_shutdown.recv() => Ok(ConnectionOutcome::Stop),
                    it = hb_task => {
                        if let Err(_) = it { eprintln!("Diagnostic: danmaku.rs:200 (details omitted)"); }
                        Ok(ConnectionOutcome::Disconnected)
                    }
                    it = recv_task => {
                        if let Err(_) = it { eprintln!("Diagnostic: danmaku.rs:204 (details omitted)"); }
                        Ok(ConnectionOutcome::Disconnected)
                    }
                }
            }
            .await;

            match result {
                Ok(ConnectionOutcome::Stop) => break,
                Ok(ConnectionOutcome::Disconnected) => {
                    if let Err(_emit_error) = app_handle_clone.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id_clone, "platform": "huya", "status": "error", "message": "弹幕连接断开，正在重连"
                    })) { log::error!("Diagnostic: danmaku.rs:216 (details omitted)"); }
                    eprintln!("Diagnostic: danmaku.rs:217 (details omitted)");
                }
                Err(_error) => {
                    if let Err(_emit_error) = app_handle_clone.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id_clone, "platform": "huya", "status": "error", "message": "平台弹幕暂不可用，请手动重试"
                    })) { log::error!("Diagnostic: danmaku.rs:225 (details omitted)"); }
                    eprintln!("Diagnostic: danmaku.rs:226 (details omitted)");
                    break;
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
    let _ = room_id; // Retained IPC argument for existing clients.
    println!("Diagnostic: danmaku.rs:250 (details omitted)");

    // 取出当前监听的停止信号发送器
    let tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };

    if let Some(tx) = tx {
        if let Err(_) = tx.send(()).await {
            println!("Diagnostic: danmaku.rs:263 (details omitted)");
        } else {
            println!("Diagnostic: danmaku.rs:265 (details omitted)");
        }
    } else {
        println!("Diagnostic: danmaku.rs:268 (details omitted)");
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
        println!("Diagnostic: danmaku.rs:348 (details omitted)");
        info!("Diagnostic: danmaku.rs:349 (details omitted)");
        return Ok(ret);
    }
    let b1 = ios.read_bytes(1, false, Default::default())?;
    let mut inner = TarsDecoder::from(b1.as_ref());
    let nested = inner.read_int32(1, false, -1).unwrap_or(-1);
    let b2 = inner.read_bytes(2, false, Default::default())?;
    println!("Diagnostic: danmaku.rs:356 (details omitted)");
    info!("Diagnostic: danmaku.rs:357 (details omitted)");
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
            println!("Diagnostic: danmaku.rs:391 (details omitted)");
            info!("Diagnostic: danmaku.rs:395 (details omitted)");
            ret = Some((nick, text));
        } else {
            println!("Diagnostic: danmaku.rs:401 (details omitted)");
            info!("Diagnostic: danmaku.rs:402 (details omitted)");
        }
    } else {
        println!("Diagnostic: danmaku.rs:405 (details omitted)");
        info!("Diagnostic: danmaku.rs:406 (details omitted)");
    }
    Ok(ret)
}

