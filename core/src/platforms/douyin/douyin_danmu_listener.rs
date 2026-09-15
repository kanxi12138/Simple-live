use tauri::Emitter;
use crate::platforms::douyin::web_api::normalize_douyin_live_id;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::{sleep, Duration};

enum ConnectionOutcome {
    Stop,
    Disconnected,
}

#[tauri::command]
pub async fn start_douyin_danmu_listener(
    payload: crate::platforms::common::GetStreamUrlPayload,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::platforms::common::DouyinDanmakuState>,
) -> Result<(), String> {
    let room_id_or_url = payload.args.room_id_str;
    println!("Diagnostic: douyin_danmu_listener.rs:18 (details omitted)");

    let previous_tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };

    if let Some(tx) = previous_tx {
        println!("Diagnostic: douyin_danmu_listener.rs:29 (details omitted)");
        if tx.send(()).await.is_err() {
            eprintln!("Diagnostic: douyin_danmu_listener.rs:31 (details omitted)");
        }
    }

    if room_id_or_url == "stop_listening" {
        println!("Diagnostic: douyin_danmu_listener.rs:36 (details omitted)");
        return Ok(());
    }

    let normalized_room_id = normalize_douyin_live_id(&room_id_or_url);

    let (tx_shutdown, mut rx_shutdown) = tokio_mpsc::channel::<()>(1);
    {
        let mut lock = state.inner().0.lock().unwrap();
        *lock = Some(tx_shutdown);
    }

    let app_handle_clone = app_handle.clone();
    let room_id_str_clone = normalized_room_id.clone();

        tokio::spawn(async move {
        println!("Diagnostic: douyin_danmu_listener.rs:54 (details omitted)");

        let mut backoff_secs = 1u64;

        for _attempt in 0..3 {
            let result = async {
                let mut fetcher = crate::platforms::douyin::danmu::web_fetcher::DouyinLiveWebFetcher::new(&room_id_str_clone)?;
                tokio::select! {
                    _ = rx_shutdown.recv() => return Ok(ConnectionOutcome::Stop),
                    result = fetcher.fetch_room_details() => result?,
                }

                let actual_room_id = fetcher.get_room_id().await?;
                let cookie_header = fetcher.get_dy_cookie().await?;
                let user_unique_id = fetcher.get_user_unique_id().await?;
                println!("Diagnostic: douyin_danmu_listener.rs:72 (details omitted)");

                let (read_stream, ack_tx, shutdown_tx) = tokio::select! {
                    _ = rx_shutdown.recv() => return Ok(ConnectionOutcome::Stop),
                    result = crate::platforms::douyin::danmu::websocket_connection::connect_and_manage_websocket(
                    &fetcher,
                    &actual_room_id,
                    &cookie_header,
                    &user_unique_id,
                ) => result?,
                };

                println!("Diagnostic: douyin_danmu_listener.rs:87 (details omitted)");

                let shutdown_tx_for_msg = shutdown_tx.clone();
                tokio::select! {
                    res = crate::platforms::douyin::danmu::message_handler::handle_received_messages(
                        read_stream,
                        ack_tx,
                        app_handle_clone.clone(),
                        room_id_str_clone.clone()
                    ) => {
                        let _ = shutdown_tx_for_msg.send(true);
                        if let Err(e) = res {
                            return Err(e);
                        }
                        Ok(ConnectionOutcome::Disconnected)
                    }
                    _ = rx_shutdown.recv() => {
                        println!("Diagnostic: douyin_danmu_listener.rs:107 (details omitted)");
                        let _ = shutdown_tx.send(true);
                        Ok(ConnectionOutcome::Stop)
                    }
                }
            }
            .await;

            match result {
                Ok(ConnectionOutcome::Stop) => break,
                Ok(ConnectionOutcome::Disconnected) => {
                    if let Err(_emit_error) = app_handle_clone.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id_str_clone, "platform": "douyin", "status": "error", "message": "弹幕连接断开，正在重连"
                    })) { log::error!("Diagnostic: douyin_danmu_listener.rs:123 (details omitted)"); }
                    eprintln!("Diagnostic: douyin_danmu_listener.rs:124 (details omitted)");
                }
                Err(_error) => {
                    if let Err(_emit_error) = app_handle_clone.emit("danmaku-status", serde_json::json!({
                        "room_id": room_id_str_clone, "platform": "douyin", "status": "error", "message": "平台弹幕暂不可用，请手动重试"
                    })) { log::error!("Diagnostic: douyin_danmu_listener.rs:132 (details omitted)"); }
                    eprintln!("Diagnostic: douyin_danmu_listener.rs:133 (details omitted)");
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

