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
    println!(
        "[Douyin Danmaku] Received request for room_id_or_url: {}",
        room_id_or_url
    );

    let previous_tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };

    if let Some(tx) = previous_tx {
        println!("[Douyin Danmaku] Sending shutdown to previous Douyin listener task.");
        if tx.send(()).await.is_err() {
            eprintln!("[Douyin Danmaku] Failed to send shutdown. Task might have already completed or panicked.");
        }
    }

    if room_id_or_url == "stop_listening" {
        println!(
            "[Douyin Danmaku] Received stop_listening signal. Listener will not be restarted."
        );
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
        println!(
            "[Douyin Danmaku] Spawning listener for room: {}",
            room_id_str_clone
        );

        let mut backoff_secs = 1u64;

        loop {
            let result = async {
                let mut fetcher = crate::platforms::douyin::danmu::web_fetcher::DouyinLiveWebFetcher::new(&room_id_str_clone)?;
                fetcher
                    .fetch_room_details()
                    .await
                    .map_err(|e| format!("Failed to fetch room details: {}", e))?;

                let actual_room_id = fetcher.get_room_id().await?;
                let cookie_header = fetcher.get_dy_cookie().await?;
                let user_unique_id = fetcher.get_user_unique_id().await?;
                println!(
                    "[Douyin Danmaku] Using: room_id={}, user_unique_id={}",
                    actual_room_id, user_unique_id
                );

                let (read_stream, ack_tx, shutdown_tx) = crate::platforms::douyin::danmu::websocket_connection::connect_and_manage_websocket(
                    &fetcher,
                    &actual_room_id,
                    &cookie_header,
                    &user_unique_id,
                )
                .await?;

                println!(
                    "[Douyin Danmaku] WebSocket connected for room: {}",
                    actual_room_id
                );

                let shutdown_tx_for_msg = shutdown_tx.clone();
                tokio::select! {
                    res = crate::platforms::douyin::danmu::message_handler::handle_received_messages(
                        read_stream,
                        ack_tx,
                        app_handle_clone.clone(),
                        actual_room_id.clone()
                    ) => {
                        let _ = shutdown_tx_for_msg.send(true);
                        if let Err(e) = res {
                            return Err(e);
                        }
                        Ok(ConnectionOutcome::Disconnected)
                    }
                    _ = rx_shutdown.recv() => {
                        println!(
                            "[Douyin Danmaku] Received shutdown signal for room {}.",
                            actual_room_id
                        );
                        let _ = shutdown_tx.send(true);
                        Ok(ConnectionOutcome::Stop)
                    }
                }
            }
            .await;

            match result {
                Ok(ConnectionOutcome::Stop) => break,
                Ok(ConnectionOutcome::Disconnected) => {
                    eprintln!(
                        "[Douyin Danmaku] Disconnected, retrying in {}s.",
                        backoff_secs
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[Douyin Danmaku] Connection error: {}. Retrying in {}s.",
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

