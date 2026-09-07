// Protocol adapted from dart_simple_live ba828e6 douyin_danmaku.dart (GPL-3.0).
use std::io::Read;
use flate2::read::GzDecoder;
use futures_util::{stream::SplitStream, StreamExt};
use prost::Message as ProstMessage;
use tauri::Emitter;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

use super::gen::{PushFrame, Response};
use super::message_parsers;
use super::websocket_connection::WsStream;

const MAX_PAYLOAD: u64 = 16 * 1024 * 1024;

fn decode_response(frame: &PushFrame) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
    let payload = if frame.payload.starts_with(&[0x1f, 0x8b]) {
        let mut decoded = Vec::new();
        GzDecoder::new(frame.payload.as_slice()).take(MAX_PAYLOAD + 1).read_to_end(&mut decoded)?;
        decoded
    } else { frame.payload.clone() };
    if payload.len() as u64 > MAX_PAYLOAD { return Err("抖音弹幕解压数据过大".into()); }
    Ok(Response::decode(payload.as_slice())?)
}

/// Receives Protobuf messages, acknowledges delivery and emits chat for the requested room.
/// Returns transport, gzip, Protobuf and ACK failures to the reconnect/error handler.
pub async fn handle_received_messages(
    mut read_stream: SplitStream<WsStream>,
    ack_tx: Sender<Message>,
    app_handle: tauri::AppHandle,
    room_id: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut authenticated = false;
    while let Some(message) = read_stream.next().await {
        match message? {
            Message::Binary(data) => {
                let frame = PushFrame::decode(data.as_slice())?;
                if frame.payload_type != "msg" || frame.payload.is_empty() { continue; }
                let response = decode_response(&frame)?;
                if response.need_ack {
                    let ack = PushFrame {
                        log_id: frame.log_id, payload_type: "ack".to_string(),
                        payload: response.internal_ext.as_bytes().to_vec(), ..Default::default()
                    };
                    ack_tx.send(Message::Binary(ack.encode_to_vec())).await?;
                }
                if !authenticated {
                    app_handle.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id, "platform": "douyin", "status": "ready"
                    }))?;
                    authenticated = true;
                }
                for message in response.messages_list {
                    if message.method != "WebcastChatMessage" { continue; }
                    if let Some(chat) = message_parsers::parse_chat_message(&message.payload, &room_id)? {
                        app_handle.emit("danmaku-message", chat)?;
                    }
                }
            }
            Message::Ping(data) => ack_tx.send(Message::Pong(data)).await?,
            Message::Close(_) => return Err("抖音弹幕连接已关闭".into()),
            _ => {},
        }
    }
    Err("抖音弹幕连接已关闭".into())
}
