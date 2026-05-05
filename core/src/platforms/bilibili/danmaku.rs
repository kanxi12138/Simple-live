use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::panic::AssertUnwindSafe;
use tauri::Emitter;
use tokio::sync::mpsc as tokio_mpsc;

use crate::platforms::bilibili::models::BiliMessage;
use crate::platforms::bilibili::websocket::BiliLiveClient;

#[tauri::command]
pub async fn start_bilibili_danmaku_listener(
    payload: crate::platforms::common::GetStreamUrlPayload,
    cookie: Option<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::platforms::common::BilibiliDanmakuState>,
) -> Result<(), String> {
    let room_id = payload.args.room_id_str.clone();
    eprintln!(
        "[Bilibili Danmaku] Starting listener for requested room {}",
        room_id
    );

    let previous_tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };
    if let Some(tx) = previous_tx {
        if tx.send(()).await.is_err() {
            eprintln!("[Bilibili Danmaku] Failed to stop previous listener task");
        }
    }

    let (tx_shutdown, mut rx_shutdown) = tokio_mpsc::channel::<()>(1);
    {
        let mut lock = state.inner().0.lock().unwrap();
        *lock = Some(tx_shutdown);
    }

    let app_handle_clone = app_handle.clone();
    let room_id_clone = room_id.clone();
    let cookie_clone = cookie.clone();

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_for_thread = stop_flag.clone();

    std::thread::spawn(move || {
        let room_id_for_thread = room_id_clone.clone();
        let panic_room_id = room_id_for_thread.clone();
        let run_result = std::panic::catch_unwind(AssertUnwindSafe(move || {
            let mut client = match cookie_clone.as_ref() {
                Some(c) => BiliLiveClient::new_with_cookie(c.as_str(), room_id_clone.as_str()),
                None => BiliLiveClient::new_without_cookie(room_id_clone.as_str()),
            };
            client.send_auth();
            eprintln!(
                "[Bilibili Danmaku] Auth packet sent for requested room {}",
                room_id_for_thread
            );

            let mut logged_first_danmu = false;
            let mut logged_first_unsupported = false;
            loop {
                if stop_flag_for_thread.load(Ordering::Relaxed) {
                    break;
                }

                if let Some(msg) = client.read_once() {
                    match msg {
                        BiliMessage::Danmu { user, text } => {
                            if !logged_first_danmu {
                                eprintln!(
                                    "[Bilibili Danmaku] Received first DANMU_MSG for requested room {} from {}",
                                    room_id_for_thread, user
                                );
                                logged_first_danmu = true;
                            }
                            let _ = app_handle_clone.emit(
                                "danmaku-message",
                                crate::platforms::common::DanmakuFrontendPayload {
                                    room_id: room_id_for_thread.clone(),
                                    user,
                                    content: text,
                                    user_level: 0,
                                    fans_club_level: 0,
                                },
                            );
                        }
                        BiliMessage::Gift { user, gift } => {
                            let _ = app_handle_clone.emit(
                                "danmaku-message",
                                crate::platforms::common::DanmakuFrontendPayload {
                                    room_id: room_id_for_thread.clone(),
                                    user,
                                    content: format!("[绀肩墿] {}", gift),
                                    user_level: 0,
                                    fans_club_level: 0,
                                },
                            );
                        }
                        BiliMessage::Activity { user, text } => {
                            let content = if text.is_empty() {
                                user.clone()
                            } else if user == "<unknown>" {
                                text.clone()
                            } else {
                                format!("{} {}", user, text)
                            };
                            let _ = app_handle_clone.emit(
                                "danmaku-message",
                                crate::platforms::common::DanmakuFrontendPayload {
                                    room_id: room_id_for_thread.clone(),
                                    user,
                                    content,
                                    user_level: 0,
                                    fans_club_level: 0,
                                },
                            );
                        }
                        BiliMessage::Unsupported { cmd } => {
                            if !logged_first_unsupported {
                                eprintln!(
                                    "[Bilibili Danmaku] Received first non-danmu cmd for requested room {}: {}",
                                    room_id_for_thread, cmd
                                );
                                logged_first_unsupported = true;
                            }
                        }
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }));

        if run_result.is_err() {
            eprintln!(
                "[Bilibili Danmaku] Listener thread panicked for requested room {}",
                panic_room_id
            );
        }
    });

    let stop_flag_for_task = stop_flag.clone();
    tokio::spawn(async move {
        let _ = rx_shutdown.recv().await;
        stop_flag_for_task.store(true, Ordering::Relaxed);
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_bilibili_danmaku_listener(
    state: tauri::State<'_, crate::platforms::common::BilibiliDanmakuState>,
) -> Result<(), String> {
    let previous_tx = {
        let mut lock = state.inner().0.lock().unwrap();
        lock.take()
    };
    if let Some(tx) = previous_tx {
        match tx.send(()).await {
            Ok(()) => Ok(()),
            Err(_) => Err("停止Bilibili弹幕监听失败：接收方已关闭".to_string()),
        }
    } else {
        Ok(())
    }
}
