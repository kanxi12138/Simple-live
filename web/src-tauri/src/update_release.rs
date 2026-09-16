use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use std::time::Duration;

static DOWNLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn allowed_download_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.port_or_known_default() == Some(443)
        && url.username().is_empty() && url.password().is_none()
        && matches!(url.host_str(), Some("github.com" | "release-assets.githubusercontent.com"))
}

fn allowed_redirect(url: &reqwest::Url, previous_count: usize) -> bool {
    previous_count <= 5 && allowed_download_url(url)
}

fn update_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().no_proxy().connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if !allowed_redirect(attempt.url(), attempt.previous().len()) {
                attempt.stop()
            } else { attempt.follow() }
        })).build().map_err(|_| "更新客户端初始化失败".into())
}

const RELEASES_API: &str = "https://api.github.com/repos/kanxi12138/Simple-live/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/kanxi12138/Simple-live/releases";
const APK_CONTENT_TYPE: &str = "application/vnd.android.package-archive";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAssetInfo {
    pub name: String,
    pub content_type: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestReleaseInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_tag: String,
    pub published_at: String,
    pub has_update: bool,
    pub html_url: String,
    pub apk_asset: Option<ReleaseAssetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkDownloadResult {
    pub version: String,
    pub file_name: String,
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAssetPayload {
    name: Option<String>,
    content_type: Option<String>,
    browser_download_url: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GithubLatestReleasePayload {
    tag_name: Option<String>,
    name: Option<String>,
    html_url: Option<String>,
    published_at: Option<String>,
    assets: Option<Vec<GithubReleaseAssetPayload>>,
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_string()
}

fn compare_versions(left: &str, right: &str) -> i32 {
    let left_parts: Vec<i32> = normalize_version(left)
        .split('.')
        .map(|part| part.parse::<i32>().unwrap_or(0))
        .collect();
    let right_parts: Vec<i32> = normalize_version(right)
        .split('.')
        .map(|part| part.parse::<i32>().unwrap_or(0))
        .collect();
    let max_length = left_parts.len().max(right_parts.len());

    for index in 0..max_length {
        let left_value = *left_parts.get(index).unwrap_or(&0);
        let right_value = *right_parts.get(index).unwrap_or(&0);
        if left_value != right_value {
            return left_value - right_value;
        }
    }

    0
}

fn select_apk_asset(assets: Option<Vec<GithubReleaseAssetPayload>>) -> Option<ReleaseAssetInfo> {
    let list = assets?;
    let matched = list
        .iter()
        .find(|asset| asset.content_type.as_deref() == Some(APK_CONTENT_TYPE))
        .or_else(|| {
            list.iter().find(|asset| {
                asset.name
                    .as_deref()
                    .map(|name| name.to_ascii_lowercase().ends_with(".apk"))
                    .unwrap_or(false)
            })
        })?;

    Some(ReleaseAssetInfo {
        name: matched.name.clone()?,
        content_type: matched
            .content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        download_url: matched.browser_download_url.clone()?,
        size: matched.size.unwrap_or(0),
    })
}

async fn fetch_latest_release_payload(client: &reqwest::Client) -> Result<GithubLatestReleasePayload, String> {
    let response = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Simple-live")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| "发布信息请求失败或超时".to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to request latest release: HTTP {}",
            response.status()
        ));
    }

    response
        .json::<GithubLatestReleasePayload>()
        .await
        .map_err(|_| "发布信息格式无效".to_string())
}

fn build_latest_release_info(payload: GithubLatestReleasePayload) -> Result<LatestReleaseInfo, String> {
    let current_version = normalize_version(env!("CARGO_PKG_VERSION"));
    let latest_version = normalize_version(
        payload
            .tag_name
            .as_deref()
            .or(payload.name.as_deref())
            .unwrap_or_default(),
    );

    if latest_version.is_empty() {
        return Err("Latest release version is missing.".to_string());
    }

    let release_tag = payload
        .tag_name
        .clone()
        .unwrap_or_else(|| format!("v{latest_version}"));
    let apk_asset = select_apk_asset(payload.assets);

    Ok(LatestReleaseInfo {
        current_version: current_version.clone(),
        latest_version: latest_version.clone(),
        release_tag,
        published_at: payload.published_at.unwrap_or_default(),
        has_update: compare_versions(&latest_version, &current_version) > 0,
        html_url: payload.html_url.unwrap_or_else(|| RELEASES_PAGE.to_string()),
        apk_asset,
    })
}

fn sanitize_file_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn build_download_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Failed to resolve cache directory: {error}"))?;
    let updates_dir = cache_dir.join("updates");
    let sanitized_file_name = sanitize_file_name(file_name);
    if !sanitized_file_name.to_ascii_lowercase().ends_with(".apk") { return Err("更新资产不是 APK 文件".into()); }
    Ok(updates_dir.join(sanitized_file_name))
}

async fn download_to_path(client: &reqwest::Client, url: &str, expected_size: u64,
    temporary: &Path, destination: &Path, idle_timeout: Duration) -> Result<(), String> {
    let result = download_bytes(client, url, expected_size, temporary, destination, idle_timeout).await;
    if result.is_err() {
        match fs::remove_file(temporary).await {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return Err("更新失败，临时文件清理失败".into()),
        }
    }
    result
}

async fn download_bytes(client: &reqwest::Client, url: &str, expected_size: u64,
    temporary: &Path, destination: &Path, idle_timeout: Duration) -> Result<(), String> {
    let response = tokio::time::timeout(idle_timeout, client
        .get(url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", "Simple-live")
        .send()).await.map_err(|_| "更新下载响应超时")?
        .map_err(|_| "更新下载连接失败")?;

    if !response.status().is_success() {
        return Err(format!("Failed to download APK: HTTP {}", response.status()));
    }

    let mut file = fs::File::create(temporary)
        .await
        .map_err(|_| "无法创建更新临时文件")?;
    let mut stream = response.bytes_stream();

    let mut received = 0u64;
    while let Some(next_chunk) = tokio::time::timeout(idle_timeout, stream.next()).await
        .map_err(|_| "更新下载连续 60 秒无数据")? {
        let chunk = next_chunk.map_err(|_| "更新下载中断")?;
        received += chunk.len() as u64;
        if received > expected_size { return Err("更新文件超过发布大小".to_string()); }
        file.write_all(&chunk)
            .await
            .map_err(|_| "更新文件写入失败")?;
    }

    file.flush()
        .await
        .map_err(|_| "更新文件刷新失败")?;
    if received != expected_size { return Err("更新文件大小不匹配".into()); }
    file.sync_all().await.map_err(|_| "更新文件保存失败")?;
    drop(file);
    fs::rename(temporary, destination).await.map_err(|_| "更新文件替换失败，原文件已保留".to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn fetch_latest_release_info_cmd(
) -> Result<LatestReleaseInfo, String> {
    let payload = fetch_latest_release_payload(&update_client()?).await?;
    build_latest_release_info(payload)
}

#[tauri::command]
pub async fn download_release_apk_cmd(
    app: AppHandle,
) -> Result<ApkDownloadResult, String> {
    let _guard = DOWNLOAD_LOCK.try_lock().map_err(|_| "已有更新下载正在进行".to_string())?;
    let client = update_client()?;
    let payload = fetch_latest_release_payload(&client).await?;
    let release_info = build_latest_release_info(payload)?;
    let asset = release_info
        .apk_asset
        .ok_or_else(|| "Latest release does not contain an installable APK asset.".to_string())?;
    let download_path = build_download_path(&app, &asset.name)?;

    let url = reqwest::Url::parse(&asset.download_url).map_err(|_| "更新下载地址无效")?;
    if !allowed_download_url(&url) || url.host_str() != Some("github.com")
        || !url.path().starts_with("/kanxi12138/Simple-live/releases/download/") || asset.size == 0 {
        return Err("更新资产地址或大小无效".into());
    }
    let parent = download_path.parent().ok_or("更新缓存路径无效")?;
    fs::create_dir_all(parent).await.map_err(|_| "无法创建更新缓存目录")?;
    let temporary = parent.join(format!("{}.part", crate::proxy::session_id()));

    download_to_path(&client, &asset.download_url, asset.size, &temporary, &download_path, Duration::from_secs(60)).await?;

    Ok(ApkDownloadResult {
        version: release_info.latest_version,
        file_name: asset.name,
        file_path: download_path.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn download_hosts_are_exact_https_endpoints() {
        for url in ["https://github.com/a", "https://release-assets.githubusercontent.com/a"] {
            assert!(allowed_download_url(&reqwest::Url::parse(url).unwrap()));
            assert!(allowed_redirect(&reqwest::Url::parse(url).unwrap(), 5));
            assert!(!allowed_redirect(&reqwest::Url::parse(url).unwrap(), 6));
        }
        for url in ["http://github.com/a", "https://github.com:2245/a", "https://github.com.invalid/a", "https://user@github.com/a"] {
            assert!(!allowed_download_url(&reqwest::Url::parse(url).unwrap()));
        }
        let guard = DOWNLOAD_LOCK.try_lock().unwrap();
        assert!(DOWNLOAD_LOCK.try_lock().is_err());
        drop(guard);
    }

    async fn local_response(body: &'static str, delay: Duration) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let _ = socket.read(&mut request).await;
            tokio::time::sleep(delay).await;
            let _ = socket.write_all(body.as_bytes()).await;
        });
        format!("http://{address}/file")
    }

    #[tokio::test]
    async fn download_failures_preserve_previous_file() {
        let directory = std::env::temp_dir().join(crate::proxy::session_id());
        fs::create_dir_all(&directory).await.unwrap();
        let destination = directory.join("release.apk");
        let temporary = directory.join("partial");
        fs::write(&destination, b"old").await.unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for (body, size, delay) in [
            ("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nx", 5, Duration::ZERO),
            ("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nnew", 2, Duration::ZERO),
            ("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nnew", 3, Duration::from_millis(200)),
        ] {
            let url = local_response(body, delay).await;
            assert!(download_to_path(&client, &url, size, &temporary, &destination, Duration::from_millis(50)).await.is_err());
            assert_eq!(fs::read(&destination).await.unwrap(), b"old");
            assert!(!temporary.exists());
        }
        let url = local_response("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nnew", Duration::ZERO).await;
        assert!(download_to_path(&client, &url, 3, &directory.join("missing/file"), &destination, Duration::from_secs(1)).await.is_err());
        assert_eq!(fs::read(&destination).await.unwrap(), b"old");
        let url = local_response("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nnew", Duration::ZERO).await;
        download_to_path(&client, &url, 3, &temporary, &destination, Duration::from_secs(1)).await.unwrap();
        assert_eq!(fs::read(&destination).await.unwrap(), b"new");
        fs::remove_dir_all(&directory).await.unwrap();
    }

    #[tokio::test]
    async fn body_idle_and_replacement_failure_cleanup() {
        let directory = std::env::temp_dir().join(crate::proxy::session_id());
        fs::create_dir_all(&directory).await.unwrap();
        let temporary = directory.join("partial");
        let destination = directory.join("release.apk");
        fs::write(&destination, b"old").await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let _ = socket.read(&mut request).await;
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nn").await.unwrap();
            tokio::time::sleep(Duration::from_secs(2)).await;
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let error = download_to_path(&client, &format!("http://{address}/file"), 3, &temporary,
            &destination, Duration::from_millis(500)).await.unwrap_err();
        assert!(error.contains("无数据"), "{error}");
        assert!(!temporary.exists());
        assert_eq!(fs::read(&destination).await.unwrap(), b"old");
        let blocked_destination = directory.join("existing-directory");
        fs::create_dir(&blocked_destination).await.unwrap();
        fs::write(blocked_destination.join("keep"), b"old").await.unwrap();
        let url = local_response("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nnew", Duration::ZERO).await;
        let error = download_to_path(&client, &url, 3, &temporary, &blocked_destination, Duration::from_secs(1)).await.unwrap_err();
        assert!(error.contains("替换失败"));
        assert!(!temporary.exists());
        assert_eq!(fs::read(blocked_destination.join("keep")).await.unwrap(), b"old");
        fs::remove_dir_all(&directory).await.unwrap();
    }
}
