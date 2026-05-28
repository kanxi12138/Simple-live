use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use tauri::{Emitter, Window};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

const DOUYU_DANMAKU_URL: &str = "wss://danmuproxy.douyu.com:8506";
const DOUYU_PACKET_HEADER_SIZE: usize = 12;
const DOUYU_PACKET_MIN_BODY_SIZE: usize = 9;
const DOUYU_HEARTBEAT_SECS: u64 = 45;
const DOUYU_LOGIN_PACKET_TYPE: u16 = 689;

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

    fn encode_message(message: &str) -> Vec<u8> {
        let message_bytes = message.as_bytes();
        let packet_length = message_bytes.len() + 9;
        let mut buffer = Vec::with_capacity(packet_length + 4);

        buffer.extend_from_slice(&(packet_length as u32).to_le_bytes());
        buffer.extend_from_slice(&(packet_length as u32).to_le_bytes());
        buffer.extend_from_slice(&DOUYU_LOGIN_PACKET_TYPE.to_le_bytes());
        buffer.push(0);
        buffer.push(0);
        buffer.extend_from_slice(message_bytes);
        buffer.push(0);

        buffer
    }

    fn decode_messages(data: &[u8]) -> (Vec<HashMap<String, String>>, usize) {
        let mut messages = Vec::new();
        let mut cursor = 0usize;

        while cursor + DOUYU_PACKET_HEADER_SIZE <= data.len() {
            let packet_length = u32::from_le_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]) as usize;

            if packet_length < DOUYU_PACKET_MIN_BODY_SIZE {
                break;
            }

            let frame_end = cursor + 4 + packet_length;
            if frame_end > data.len() {
                break;
            }

            let body_start = cursor + DOUYU_PACKET_HEADER_SIZE;
            let body_length = packet_length.saturating_sub(9);
            let body_end = body_start + body_length;
            if body_start < body_end && body_end <= data.len() {
                let body = String::from_utf8_lossy(&data[body_start..body_end]);
                for message in Self::parse_stt_batch(&body) {
                    messages.push(message);
                }
            }

            cursor = frame_end;
        }

        (messages, cursor)
    }

    fn parse_stt_batch(message: &str) -> Vec<HashMap<String, String>> {
        message
            .trim_end_matches('\0')
            .split("//")
            .filter_map(Self::parse_stt_message)
            .collect()
    }

    fn parse_stt_message(message: &str) -> Option<HashMap<String, String>> {
        let mut result = HashMap::new();

        for field in message.split('/') {
            if field.is_empty() {
                continue;
            }

            if let Some((key, value)) = field.split_once("@=") {
                result.insert(
                    key.to_string(),
                    value.replace("@S", "/").replace("@A", "@"),
                );
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    async fn run_connection(
        &self,
        stop_rx: &mut oneshot::Receiver<()>,
    ) -> Result<ConnectionOutcome, Box<dyn std::error::Error>> {
        eprintln!(
            "[Douyu Danmaku {}] Connecting websocket to danmuproxy.douyu.com:8506",
            self.room_id
        );
        let url = Url::parse(DOUYU_DANMAKU_URL)?;
        let (ws_stream, _) = timeout(Duration::from_secs(10), connect_async(url.as_str()))
            .await
            .map_err(|_| "Douyu websocket connect timeout")??;
        eprintln!("[Douyu Danmaku {}] Websocket connected", self.room_id);

        let (mut write, mut read) = ws_stream.split();
        let login_message = format!("type@=loginreq/roomid@={}/", self.room_id);
        let join_message = format!("type@=joingroup/rid@={}/gid@=-9999/", self.room_id);

        write
            .send(Message::Binary(Self::encode_message(&login_message).into()))
            .await?;
        write
            .send(Message::Binary(Self::encode_message(&join_message).into()))
            .await?;

        let (send_tx, mut send_rx) = mpsc::channel::<Message>(16);
        let heartbeat_frame = Self::encode_message("type@=mrkl/");
        let heartbeat_tx = send_tx.clone();

        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(DOUYU_HEARTBEAT_SECS)).await;
                if heartbeat_tx
                    .send(Message::Binary(heartbeat_frame.clone().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        let send_task = tokio::spawn(async move {
            while let Some(message) = send_rx.recv().await {
                if write.send(message).await.is_err() {
                    break;
                }
            }
        });

        let room_id = self.room_id.clone();
        let window = self.window.clone();
        let mut has_logged_first_type = false;
        let mut pending_binary_buffer = Vec::<u8>::new();

        loop {
            tokio::select! {
                _ = &mut *stop_rx => {
                    eprintln!("[Douyu Danmaku {}] Stop signal received", room_id);
                    send_task.abort();
                    return Ok(ConnectionOutcome::Stop);
                }
                message = read.next() => {
                    match message {
                        Some(Ok(Message::Binary(data))) => {
                            pending_binary_buffer.extend_from_slice(&data);
                            Self::handle_packet_batch(
                                &window,
                                &room_id,
                                &mut pending_binary_buffer,
                                &mut has_logged_first_type,
                            );
                        }
                        Some(Ok(Message::Text(text))) => {
                            if let Some(parsed) = Self::parse_stt_message(text.trim_end_matches('\0')) {
                                Self::emit_chat_message(&window, &room_id, &parsed, &mut has_logged_first_type);
                            }
                        }
                        Some(Ok(Message::Close(frame))) => {
                            eprintln!("[Douyu Danmaku {}] Websocket closed: {:?}", room_id, frame);
                            send_task.abort();
                            return Ok(ConnectionOutcome::Disconnected);
                        }
                        Some(Err(error)) => {
                            eprintln!("[Douyu Danmaku {}] Websocket error: {}", room_id, error);
                            send_task.abort();
                            return Ok(ConnectionOutcome::Disconnected);
                        }
                        None => {
                            eprintln!("[Douyu Danmaku {}] Websocket stream ended", room_id);
                            send_task.abort();
                            return Ok(ConnectionOutcome::Disconnected);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_packet_batch(
        window: &Window,
        room_id: &str,
        data: &mut Vec<u8>,
        has_logged_first_type: &mut bool,
    ) {
        let (messages, consumed_len) = Self::decode_messages(data);
        for message in messages {
            Self::emit_chat_message(window, room_id, &message, has_logged_first_type);
        }
        if consumed_len > 0 {
            data.drain(0..consumed_len);
        }
    }

    fn emit_chat_message(
        window: &Window,
        room_id: &str,
        message: &HashMap<String, String>,
        has_logged_first_type: &mut bool,
    ) {
        if !*has_logged_first_type {
            if let Some(message_type) = message.get("type") {
                eprintln!(
                    "[Douyu Danmaku {}] Received first message type {}",
                    room_id, message_type
                );
                *has_logged_first_type = true;
            }
        }

        if message.get("type").map(String::as_str) != Some("chatmsg") {
            return;
        }

        let unknown = "unknown".to_string();
        let empty = "".to_string();
        let zero = "0".to_string();
        let event_name = format!("danmaku-{room_id}");

        let danmaku = serde_json::json!({
            "type": "chatmsg",
            "nickname": message.get("nn").unwrap_or(&unknown),
            "content": message.get("txt").unwrap_or(&empty),
            "level": message.get("level").unwrap_or(&zero),
            "badgeName": message.get("bnn").unwrap_or(&empty),
            "badgeLevel": message.get("bl").unwrap_or(&zero),
            "color": message.get("col").map(|value| value.to_string()),
            "room_id": room_id.to_string()
        });

        let _ = window.emit(&event_name, danmaku);
        let _ = window.emit(
            "danmaku-message",
            crate::platforms::common::DanmakuFrontendPayload {
                room_id: room_id.to_string(),
                user: message.get("nn").unwrap_or(&unknown).to_string(),
                content: message.get("txt").unwrap_or(&empty).to_string(),
                user_level: message
                    .get("level")
                    .unwrap_or(&zero)
                    .parse::<i64>()
                    .unwrap_or(0),
                fans_club_level: message
                    .get("bl")
                    .unwrap_or(&zero)
                    .parse::<i32>()
                    .unwrap_or(0),
            },
        );
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut stop_rx = std::mem::replace(&mut self.stop_signal_rx, oneshot::channel().1);
        let mut backoff_secs = 1u64;

        loop {
            let outcome = self.run_connection(&mut stop_rx).await?;
            match outcome {
                ConnectionOutcome::Stop => {
                    eprintln!("[Douyu Danmaku {}] Listener stopped", self.room_id);
                    break;
                }
                ConnectionOutcome::Disconnected => {
                    eprintln!(
                        "[Douyu Danmaku {}] Disconnected, retrying in {}s",
                        self.room_id, backoff_secs
                    );
                    let sleep_future = sleep(Duration::from_secs(backoff_secs));
                    tokio::select! {
                        _ = sleep_future => {}
                        _ = &mut stop_rx => {
                            eprintln!("[Douyu Danmaku {}] Stop signal received during backoff", self.room_id);
                            break;
                        }
                    }
                    backoff_secs = (backoff_secs * 2).min(30);
                }
            }
        }

        Ok(())
    }
}
