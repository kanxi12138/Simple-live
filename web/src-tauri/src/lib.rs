#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use reqwest;
use std::collections::HashMap;
use std::panic;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::oneshot;

mod config_transfer;
mod platforms;
mod proxy;

use platforms::common::{DouyinDanmakuState, FollowHttpClient, HuyaDanmakuState};
use platforms::douyin::danmu::signature::generate_douyin_ms_token;
use platforms::douyin::fetch_douyin_partition_rooms;
use platforms::douyin::fetch_douyin_room_info;
use platforms::douyin::fetch_douyin_streamer_info;
use platforms::douyin::search_douyin_live_rooms;
use platforms::douyin::start_douyin_danmu_listener;
use platforms::douyin::{get_douyin_live_stream_url, get_douyin_live_stream_url_with_quality};
use platforms::douyu::fetch_categories;
use platforms::douyu::fetch_douyu_room_info;
use platforms::douyu::fetch_three_cate;
use platforms::douyu::{fetch_live_list, fetch_live_list_for_cate3};
use platforms::huya::stop_huya_danmaku_listener;
use platforms::huya::{fetch_huya_live_list, start_huya_danmaku_listener};

#[derive(Default, Clone)]
pub struct StreamUrlStore {
    pub url: Arc<Mutex<String>>,
}

#[derive(Default, Clone)]
pub struct DouyuDanmakuHandles(Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>);

#[tauri::command]
async fn get_stream_url_cmd(room_id: String) -> Result<String, String> {
    platforms::douyu::get_stream_url(&room_id, None)
        .await
        .map_err(|error| format!("Failed to get stream URL: {error}"))
}

#[tauri::command]
async fn get_stream_url_with_quality_cmd(
    room_id: String,
    quality: String,
    line: Option<String>,
) -> Result<String, String> {
    platforms::douyu::get_stream_url_with_quality(&room_id, &quality, line.as_deref())
        .await
        .map_err(|error| format!("Failed to get stream URL with quality: {error}"))
}

#[tauri::command]
async fn set_stream_url_cmd(
    url: String,
    state: tauri::State<'_, StreamUrlStore>,
) -> Result<(), String> {
    let mut current_url = state.url.lock().unwrap();
    *current_url = url;
    Ok(())
}

#[tauri::command]
async fn start_danmaku_listener(
    room_id: String,
    window: tauri::Window,
    danmaku_handles: tauri::State<'_, DouyuDanmakuHandles>,
) -> Result<(), String> {
    if let Some(existing_sender) = danmaku_handles
        .0
        .lock()
        .unwrap()
        .remove(&room_id)
    {
        let _ = existing_sender.send(());
    }

    let (stop_tx, stop_rx) = oneshot::channel();
    danmaku_handles
        .0
        .lock()
        .unwrap()
        .insert(room_id.clone(), stop_tx);

    let window_clone = window.clone();
    let requested_room_id = room_id.clone();
    tokio::spawn(async move {
        let normalized_room_id = match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            platforms::douyu::stream_url::fetch_douyu_room_init_cmd(requested_room_id.clone()),
        )
        .await
        {
            Ok(Ok(info)) if !info.room_id.trim().is_empty() => info.room_id,
            Ok(Ok(_)) => requested_room_id.clone(),
            Ok(Err(error)) => {
                eprintln!(
                    "[Douyu Danmaku] Failed to normalize room id {}: {}. Fallback to original id",
                    requested_room_id, error
                );
                requested_room_id.clone()
            }
            Err(_) => {
                eprintln!(
                    "[Douyu Danmaku] Timed out normalizing room id {}. Fallback to original id",
                    requested_room_id
                );
                requested_room_id.clone()
            }
        };
        let mut client = platforms::douyu::danmu_start::DanmakuClient::new(
            &normalized_room_id,
            window_clone,
            stop_rx,
        );
        if let Err(error) = client.start().await {
            eprintln!(
                "[Rust Main] Douyu danmaku client for room {} failed: {}",
                normalized_room_id, error
            );
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_danmaku_listener(
    room_id: String,
    danmaku_handles: tauri::State<'_, DouyuDanmakuHandles>,
) -> Result<(), String> {
    if let Some(sender) = danmaku_handles
        .0
        .lock()
        .unwrap()
        .remove(&room_id)
    {
        sender
            .send(())
            .map_err(|_| format!("Failed to stop Douyu danmaku listener for room {room_id}: receiver dropped."))?;
    }
    Ok(())
}

#[tauri::command]
async fn search_anchor(keyword: String) -> Result<String, String> {
    platforms::douyu::perform_anchor_search(&keyword)
        .await
        .map_err(|error| error.to_string())
}

fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Linux; Android 14; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36")
        .no_proxy()
        .build()
        .expect("Failed to create reqwest client")
}

fn build_app() -> tauri::Builder<tauri::Wry> {
    let client = create_http_client();
    let follow_http_client = FollowHttpClient::new().expect("Failed to create follow http client");

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .manage(client)
        .manage(follow_http_client)
        .manage(DouyuDanmakuHandles::default())
        .manage(DouyinDanmakuState::default())
        .manage(HuyaDanmakuState::default())
        .manage(platforms::common::BilibiliDanmakuState::default())
        .manage(StreamUrlStore::default())
        .manage(proxy::ProxyServerHandle::default())
        .manage(platforms::bilibili::state::BilibiliState::default())
        .invoke_handler(tauri::generate_handler![
            get_stream_url_cmd,
            get_stream_url_with_quality_cmd,
            set_stream_url_cmd,
            platforms::douyu::stream_url::fetch_douyu_room_init_cmd,
            platforms::douyu::stream_url::fetch_douyu_home_h5_enc_cmd,
            platforms::douyu::stream_url::fetch_douyu_play_info_cmd,
            platforms::douyu::stream_url::fetch_douyu_play_url_cmd,
            search_anchor,
            config_transfer::save_config_export,
            config_transfer::pick_config_import,
            start_danmaku_listener,
            stop_danmaku_listener,
            start_douyin_danmu_listener,
            start_huya_danmaku_listener,
            stop_huya_danmaku_listener,
            platforms::bilibili::danmaku::start_bilibili_danmaku_listener,
            platforms::bilibili::danmaku::stop_bilibili_danmaku_listener,
            proxy::start_proxy,
            proxy::stop_proxy,
            proxy::start_static_proxy_server,
            fetch_categories,
            fetch_live_list,
            fetch_live_list_for_cate3,
            fetch_douyu_room_info,
            fetch_three_cate,
            generate_douyin_ms_token,
            fetch_douyin_partition_rooms,
            get_douyin_live_stream_url,
            get_douyin_live_stream_url_with_quality,
            fetch_douyin_room_info,
            fetch_douyin_streamer_info,
            search_douyin_live_rooms,
            fetch_huya_live_list,
            platforms::huya::danmaku::fetch_huya_join_params,
            platforms::huya::stream_url::get_huya_unified_cmd,
            platforms::bilibili::state::generate_bilibili_w_webid,
            platforms::bilibili::live_list::fetch_bilibili_live_list,
            platforms::bilibili::stream_url::get_bilibili_live_stream_url_with_quality,
            platforms::bilibili::streamer_info::fetch_bilibili_streamer_info,
            platforms::bilibili::cookie::get_bilibili_cookie,
            platforms::bilibili::cookie::bootstrap_bilibili_cookie,
            platforms::bilibili::search::search_bilibili_rooms,
            platforms::huya::search::search_huya_anchors,
        ]);

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::SIZE)
                .build(),
        );
    }

    builder.setup(|app| {
        #[cfg(target_os = "macos")]
        {
            use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
            if let Some(window) = app.get_webview_window("main") {
                let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None);
            }
        }
        Ok(())
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    panic::set_hook(Box::new(|info| {
        eprintln!("[panic] {}", info);
    }));

    build_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
