use chrono::Utc;
use futures_util::{stream::SplitStream, SinkExt, StreamExt};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream};
use urlencoding;

use super::gen::PushFrame;
use super::signature;
use super::web_fetcher::DouyinLiveWebFetcher;
use prost::Message as ProstMessage;

pub type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_and_manage_websocket(
    fetcher: &DouyinLiveWebFetcher,
    room_id: &str,
    cookie_header: &str,
    user_unique_id: &str,
) -> Result<(SplitStream<WsStream>, Sender<WsMessage>, watch::Sender<bool>), Box<dyn std::error::Error + Send + Sync>> {
    let ws_cookie_header = cookie_header.to_string();
    let current_timestamp_ms = Utc::now().timestamp_millis();
    let first_req_ms = current_timestamp_ms - 100;
    let cursor = format!("h-1_t-{}_r-1_d-1_u-1", current_timestamp_ms);
    let internal_ext_original = format!(
        "internal_src:dim|wss_push_room_id:{}|wss_push_did:{}|first_req_ms:{}|fetch_time:{}|seq:1|wss_info:0-{}-0-0|wrds_v:7392094459690748497",
        room_id, user_unique_id, first_req_ms, current_timestamp_ms, current_timestamp_ms
    );
    let internal_ext_encoded = urlencoding::encode(&internal_ext_original);
    let unsigned_wss_url = format!(
        concat!(
            "wss://webcast5-ws-web-hl.douyin.com/webcast/im/push/v2/?",
            "app_name=douyin_web&version_code=180800&webcast_sdk_version=1.3.0&update_version_code=1.3.0&compress=gzip&",
            "device_platform=web&cookie_enabled=true&screen_width=1920&screen_height=1080&browser_language=zh-CN&",
            "browser_platform=Win32&browser_name=Mozilla&browser_version=5.0%20(Windows%20NT%2010.0;%20Win64;%20x64)%20AppleWebKit/537.36%20(KHTML,%20like%20Gecko)%20Chrome/126.0.0.0%20Safari/537.36&",
            "browser_online=true&tz_name=Asia/Shanghai&cursor={}&internal_ext={}&host=https://live.douyin.com&aid=6383&",
            "live_id=1&did_rule=3&endpoint=live_pc&support_wrds=1&user_unique_id={}&im_path=/webcast/im/fetch/&",
            "identity=audience&need_persist_msg_count=15&insert_task_id=&live_reason=&room_id={}&heartbeatDuration=0"
        ),
        cursor, internal_ext_encoded, user_unique_id, room_id
    );
    let signature = signature::generate_signature(&unsigned_wss_url).await?;
    let final_wss_url = format!("{unsigned_wss_url}&signature={signature}");

    let mut client_request = final_wss_url.into_client_request()?;
    let headers = client_request.headers_mut();
    headers.insert("accept", "application/json, text/plain, */*".parse()?);
    headers.insert("accept-language", "zh-CN,zh;q=0.9,en;q=0.8".parse()?);
    headers.insert("cache-control", "no-cache".parse()?);
    headers.insert("pragma", "no-cache".parse()?);
    headers.insert("origin", "https://live.douyin.com".parse()?);
    headers.insert(
        "referer",
        format!("https://live.douyin.com/{}", fetcher.live_id).parse()?,
    );
    headers.insert(
        "sec-websocket-extensions",
        "permessage-deflate; client_max_window_bits".parse()?,
    );
    headers.insert("sec-websocket-version", "13".parse()?);
    headers.insert("user-agent", fetcher.user_agent.parse()?);
    headers.insert("cookie", ws_cookie_header.parse()?);

    let (ws_stream, _response) = connect_async(client_request).await?;
    let (mut write, read) = ws_stream.split();

    let (tx, mut rx) = mpsc::channel::<WsMessage>(32);
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        let heartbeat_msg_proto = PushFrame {
            payload_type: "hb".to_string(),
            log_id: 0,
            payload: vec![],
            ..Default::default()
        };
        let mut heartbeat_buf = Vec::new();
        heartbeat_msg_proto.encode(&mut heartbeat_buf).unwrap();
        let ws_ping_msg = WsMessage::Ping(heartbeat_buf);
        let mut ticker = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                _ = ticker.tick() => {
                    if let Err(e) = write.send(ws_ping_msg.clone()).await {
                        println!("[Douyin Danmaku] Heartbeat send error: {}", e);
                        break;
                    }
                }
                msg_opt = rx.recv() => {
                    match msg_opt {
                        Some(msg_to_send) => {
                            if let Err(e) = write.send(msg_to_send).await {
                                println!("[Douyin Danmaku] Send error: {}", e);
                                if matches!(
                                    e,
                                    tokio_tungstenite::tungstenite::Error::ConnectionClosed
                                        | tokio_tungstenite::tungstenite::Error::AlreadyClosed
                                ) {
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
                else => break,
            }
        }
        println!("WebSocket send/heartbeat task ended.");
    });

    Ok((read, tx, shutdown_tx))
}
