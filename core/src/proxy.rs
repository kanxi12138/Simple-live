use actix_web::{dev::ServerHandle, web, App, HttpRequest, HttpResponse, HttpServer};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

#[derive(Default)]
pub struct ProxyServerHandle(pub Mutex<Option<ServerHandle>>);

static STATIC_SERVER: Lazy<tokio::sync::Mutex<Option<(ServerHandle, String)>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

pub(crate) fn session_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn cors() -> actix_cors::Cors {
    let cors = actix_cors::Cors::default()
        .allowed_origin("tauri://localhost")
        .allowed_origin("http://tauri.localhost")
        .allowed_origin("https://tauri.localhost")
        .allowed_origin("http://localhost")
        .allowed_origin("https://localhost")
        .allowed_methods(vec!["GET", "HEAD", "OPTIONS"])
        .allowed_headers(vec!["Range", "Content-Type"])
        .expose_headers(vec!["Content-Range", "Content-Length", "Accept-Ranges"]);
    #[cfg(debug_assertions)]
    let cors = cors.allowed_origin("http://localhost:2896").allowed_origin("http://127.0.0.1:2896");
    cors
}

fn client(timeout: u64) -> Result<Client, String> {
    Client::builder().redirect(crate::network_policy::redirects()).no_proxy().http1_only()
        .redirect(crate::network_policy::redirects())
        .gzip(false).brotli(false).no_deflate()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(timeout)).build()
        .map_err(|_| "Cannot initialize playback client".to_string())
}

#[derive(Deserialize)]
struct ImageQuery { url: String }

async fn image_proxy_handler(
    request: HttpRequest,
    query: web::Query<ImageQuery>,
    token: web::Data<String>,
    client: web::Data<Client>,
) -> HttpResponse {
    if request.match_info().get("token") != Some(token.as_str()) {
        return HttpResponse::Forbidden().finish();
    }
    let Ok(url) = reqwest::Url::parse(&query.url) else {
        return HttpResponse::BadRequest().finish();
    };
    let Some(platform) = crate::network_policy::image_platform(&url) else {
        return HttpResponse::Forbidden().finish();
    };
    let referer = match platform {
        "bilibili" => "https://live.bilibili.com/",
        "huya" => "https://www.huya.com/",
        "douyin" => "https://www.douyin.com/",
        _ => "https://www.douyu.com/",
    };
    let Ok(mut response) = client.get(url).header("Referer", referer)
        .header("User-Agent", crate::platforms::common::http_client::DEFAULT_USER_AGENT)
        .send().await else { return HttpResponse::BadGateway().finish(); };
    if !response.status().is_success() { return HttpResponse::BadGateway().finish(); }
    let content_type = response.headers().get("Content-Type")
        .and_then(|value| value.to_str().ok()).unwrap_or("").to_string();
    if !content_type.starts_with("image/") { return HttpResponse::BadGateway().finish(); }
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 8 * 1024 * 1024 => bytes.extend_from_slice(&chunk),
            Ok(None) => break,
            _ => return HttpResponse::BadGateway().finish(),
        }
    }
    HttpResponse::Ok().content_type(content_type)
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("X-Content-Type-Options", "nosniff")).body(bytes)
}

#[tauri::command]
pub async fn start_proxy(
    server_handle_state: State<'_, ProxyServerHandle>,
    stream_url_store: State<'_, crate::StreamUrlStore>,
) -> Result<String, String> {
    let source = stream_url_store.source.lock().map_err(|_| "Playback state unavailable")?.clone();
    let url = reqwest::Url::parse(&source.url).map_err(|_| "Invalid playback URL")?;
    if crate::network_policy::stream_platform(&url).is_none() { return Err("Unsupported playback host".into()); }
    let previous = server_handle_state.0.lock().map_err(|_| "Playback state unavailable")?.take();
    if let Some(previous) = previous { previous.stop(false).await; }
    let session = web::Data::new(crate::stream_proxy::StreamSession::new(source));
    let root = session.root_path();
    let client = web::Data::new(client(7200)?);
    let server = HttpServer::new(move || App::new().app_data(session.clone()).app_data(client.clone())
        .wrap(cors())
        .route("/live.flv", web::get().to(crate::stream_proxy::forward_stream))
        .route("/live.m3u8", web::get().to(crate::stream_proxy::forward_stream))
        .route("/resource", web::get().to(crate::stream_proxy::forward_stream)))
        .workers(1).bind(("127.0.0.1", 0)).map_err(|_| "Cannot bind playback service")?;
    let port = server.addrs()[0].port();
    let server = server.run();
    *server_handle_state.0.lock().map_err(|_| "Playback state unavailable")? = Some(server.handle());
    tauri::async_runtime::spawn(server);
    Ok(format!("http://127.0.0.1:{port}{root}"))
}

#[tauri::command]
pub async fn start_static_proxy_server() -> Result<String, String> {
    let mut state = STATIC_SERVER.lock().await;
    if let Some((_, base)) = state.as_ref() { return Ok(base.clone()); }
    let token = session_id();
    let session = web::Data::new(token.clone());
    let client = web::Data::new(client(20)?);
    let server = HttpServer::new(move || App::new().app_data(session.clone()).app_data(client.clone())
        .wrap(cors()).route("/{token}/image", web::get().to(image_proxy_handler)))
        .workers(1).bind(("127.0.0.1", 0)).map_err(|_| "Cannot bind image service")?;
    let base = format!("http://127.0.0.1:{}/{token}", server.addrs()[0].port());
    let server = server.run();
    *state = Some((server.handle(), base.clone()));
    tauri::async_runtime::spawn(server);
    Ok(base)
}

#[tauri::command]
pub async fn stop_proxy(
    server_handle_state: State<'_, ProxyServerHandle>,
    stream_url_store: State<'_, crate::StreamUrlStore>,
) -> Result<(), String> {
    *stream_url_store.source.lock().map_err(|_| "Playback state unavailable")? = crate::StreamSource::default();
    let previous = server_handle_state.0.lock().map_err(|_| "Playback state unavailable")?.take();
    if let Some(previous) = previous { previous.stop(false).await; }
    Ok(())
}
