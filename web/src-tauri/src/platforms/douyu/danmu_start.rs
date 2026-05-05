use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use tauri::{Emitter, Window};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

pub struct DanmakuClient {
    room_id: String,
    window: Window,
    stop_signal_rx: oneshot::Receiver<()>,
}

enum ConnectionOutcome {
    Stop,
    Disconnected,
}

impl DanmakuClient {
    pub fn new(room_id: &str, window: Window, stop_signal_rx: oneshot::Receiver<()>) -> Self {
        Self {
            room_id: room_id.to_string(),
            window,
            stop_signal_rx,
        }
    }

    fn decode_messages(&self, data: &[u8]) -> Vec<HashMap<String, String>> {
        let mut messages = Vec::new();
        let mut cursor = 0usize;

        while let Some(relative_start) = data[cursor..]
            .windows(6)
            .position(|window| window == b"type@=")
        {
            let start = cursor + relative_start;
            let Some(relative_end) = data[start..].iter().position(|byte| *byte == 0) else {
                break;
            };
            let end = start + relative_end;
            let content = String::from_utf8_lossy(&data[start..end]);
            let mut result = HashMap::new();

            for item in content.split('/') {
                if item.is_empty() {
                    continue;
                }
                if let Some((key, value)) = item.split_once("@=") {
                    result.insert(
                        key.to_string(),
                        value.replace("@S", "/").replace("@A", "@"),
                    );
                }
            }

            if !result.is_empty() {
                messages.push(result);
            }
            cursor = end + 1;
        }

        messages
    }

    fn encode_msg(&self, msg: &str) -> Vec<u8> {
        let msg_bytes = msg.as_bytes();
        let packet_len = msg_bytes.len() + 9;

        let mut result = Vec::new();
        result.extend_from_slice(&(packet_len as u32).to_le_bytes());
        result.extend_from_slice(&(packet_len as u32).to_le_bytes());
        result.extend_from_slice(&689u16.to_le_bytes());
        result.push(0);
        result.push(0);
        result.extend_from_slice(msg_bytes);
        result.push(0);

        result
    }

    async fn run_connection(
        &self,
        stop_rx: &mut oneshot::Receiver<()>,
    ) -> Result<ConnectionOutcome, Box<dyn std::error::Error>> {
        eprintln!(
            "[Douyu Danmaku {}] Connecting websocket to danmuproxy.douyu.com",
            self.room_id
        );
        let url = Url::parse("wss://danmuproxy.douyu.com:8503/")?;
        let (ws_stream, _) = timeout(Duration::from_secs(10), connect_async(url.as_str()))
            .await
            .map_err(|_| "Douyu websocket connect timeout")??;
        eprintln!("[Douyu Danmaku {}] Websocket connected", self.room_id);

        let (mut write, mut read) = ws_stream.split();

        // 发送登录请求
        let login_msg = format!("type@=loginreq/roomid@={}/", self.room_id);
        let login_data = self.encode_msg(&login_msg);
        write.send(Message::Binary(login_data)).await?;

        // 发送加入房间请求
        let join_msg = format!("type@=joingroup/rid@={}/gid@=-9999/", self.room_id);
        let join_data = self.encode_msg(&join_msg);
        write.send(Message::Binary(join_data)).await?;

        // 创建消息通道
        let (tx, mut rx) = mpsc::channel(32);

        // 启动心跳任务
        let heartbeat_msg = "type@=mrkl/";
        let heartbeat_data = self.encode_msg(heartbeat_msg);
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(45)).await;
                if let Err(_) = tx_clone.send(Message::Binary(heartbeat_data.clone())).await {
                    break;
                }
            }
        });

        // Message sending task
        let send_task = tokio::spawn(async move {
            while let Some(msg_to_send) = rx.recv().await {
                if let Err(_) = write.send(msg_to_send).await {
                    break;
                }
            }
        });

        let window = self.window.clone();
        let room_id_clone = self.room_id.clone();
        let mut logged_first_message_type = false;

        // Processing incoming messages
        loop {
            tokio::select! {
                _ = &mut *stop_rx => {
                    eprintln!("[Douyu Danmaku {}] Stop signal received, terminating listener.", room_id_clone);
                    send_task.abort();
                    return Ok(ConnectionOutcome::Stop);
                }
                msg_option = read.next() => {
                    match msg_option {
                        Some(Ok(Message::Binary(data))) => {
                            for result in self.decode_messages(&data) {
                                if !logged_first_message_type {
                                    if let Some(message_type) = result.get("type") {
                                        eprintln!(
                                            "[Douyu Danmaku {}] Received first message type {}",
                                            room_id_clone, message_type
                                        );
                                        logged_first_message_type = true;
                                    }
                                }

                                let event_name = format!("danmaku-{}", room_id_clone);

                                if result.get("type").map_or(false, |t| t == "chatmsg") {
                                    let unknown = "unknown".to_string();
                                    let empty = "".to_string();
                                    let zero = "0".to_string();

                                    let danmaku = serde_json::json!({
                                        "type": "chatmsg",
                                        "nickname": result.get("nn").unwrap_or(&unknown),
                                        "content": result.get("txt").unwrap_or(&empty),
                                        "level": result.get("level").unwrap_or(&zero),
                                        "badgeName": result.get("bnn").unwrap_or(&empty),
                                        "badgeLevel": result.get("bl").unwrap_or(&zero),
                                        "color": result.get("col").map_or(None, |c| Some(c.to_string())),
                                        "room_id": room_id_clone.clone()
                                    });

                                    let _ = window.emit(&event_name, danmaku);

                                    let _ = window.emit(
                                        "danmaku-message",
                                        crate::platforms::common::DanmakuFrontendPayload {
                                            room_id: room_id_clone.clone(),
                                            user: result.get("nn").unwrap_or(&unknown).to_string(),
                                            content: result.get("txt").unwrap_or(&empty).to_string(),
                                            user_level: result
                                                .get("level")
                                                .unwrap_or(&zero)
                                                .parse::<i64>()
                                                .unwrap_or(0),
                                            fans_club_level: result
                                                .get("bl")
                                                .unwrap_or(&zero)
                                                .parse::<i32>()
                                                .unwrap_or(0),
                                        },
                                    );
                                } else if result.get("type").map_or(false, |t| t == "uenter") {
                                    let unknown = "unknown".to_string();
                                    let empty = "".to_string();
                                    let zero = "0".to_string();

                                    let uenter_msg = serde_json::json!({
                                        "type": "uenter",
                                        "uid": result.get("uid").unwrap_or(&empty),
                                        "nickname": result.get("nn").unwrap_or(&unknown),
                                        "level": result.get("level").unwrap_or(&zero),
                                        "badgeName": result.get("bnn").unwrap_or(&empty),
                                        "badgeLevel": result.get("bl").unwrap_or(&zero),
                                        "room_id": room_id_clone.clone()
                                    });
                                    let _ = window.emit(&event_name, uenter_msg);
                                    let _ = window.emit(
                                        "danmaku-message",
                                        crate::platforms::common::DanmakuFrontendPayload {
                                            room_id: room_id_clone.clone(),
                                            user: result.get("nn").unwrap_or(&unknown).to_string(),
                                            content: "进入直播间".to_string(),
                                            user_level: result
                                                .get("level")
                                                .unwrap_or(&zero)
                                                .parse::<i64>()
                                                .unwrap_or(0),
                                            fans_club_level: result
                                                .get("bl")
                                                .unwrap_or(&zero)
                                                .parse::<i32>()
                                                .unwrap_or(0),
                                        },
                                    );
                                }
                            }
                        }
                        Some(Ok(Message::Close(frame))) => {
                            eprintln!(
                                "[Douyu Danmaku {}] Websocket closed: {:?}",
                                room_id_clone, frame
                            );
                            send_task.abort();
                            return Ok(ConnectionOutcome::Disconnected);
                        }
                        Some(Err(error)) => {
                            eprintln!(
                                "[Douyu Danmaku {}] Websocket error: {}",
                                room_id_clone, error
                            );
                            send_task.abort();
                            return Ok(ConnectionOutcome::Disconnected);
                        }
                        None => {
                            eprintln!(
                                "[Douyu Danmaku {}] Websocket stream ended",
                                room_id_clone
                            );
                            send_task.abort();
                            return Ok(ConnectionOutcome::Disconnected);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut stop_rx = std::mem::replace(&mut self.stop_signal_rx, oneshot::channel().1);
        let mut backoff_secs = 1u64;

        loop {
            let outcome = self.run_connection(&mut stop_rx).await?;
            match outcome {
                ConnectionOutcome::Stop => {
                    eprintln!("[Douyu Danmaku {}] Listener stopped.", self.room_id);
                    break;
                }
                ConnectionOutcome::Disconnected => {
                    eprintln!(
                        "[Douyu Danmaku {}] Disconnected, retrying in {}s.",
                        self.room_id, backoff_secs
                    );
                    let sleep_fut = sleep(Duration::from_secs(backoff_secs));
                    tokio::select! {
                        _ = sleep_fut => {}
                        _ = &mut stop_rx => {
                            eprintln!("[Douyu Danmaku {}] Stop signal received during backoff.", self.room_id);
                            break;
                        }
                    }
                    backoff_secs = (backoff_secs * 2).min(30);
                    continue;
                }
            }
        }
        Ok(())
    }
}
