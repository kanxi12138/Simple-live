use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

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
        .send()
        .await
        .map_err(|error| format!("Failed to request latest release: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to request latest release: HTTP {}",
            response.status()
        ));
    }

    response
        .json::<GithubLatestReleasePayload>()
        .await
        .map_err(|error| format!("Failed to parse latest release payload: {error}"))
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
    Ok(updates_dir.join(sanitized_file_name))
}

async fn prepare_download_path(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Failed to create update directory: {error}"))?;
    }

    if fs::try_exists(path)
        .await
        .map_err(|error| format!("Failed to inspect existing update file: {error}"))?
    {
        fs::remove_file(path)
            .await
            .map_err(|error| format!("Failed to remove previous update file: {error}"))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn fetch_latest_release_info_cmd(
    client: tauri::State<'_, reqwest::Client>,
) -> Result<LatestReleaseInfo, String> {
    let payload = fetch_latest_release_payload(&client).await?;
    build_latest_release_info(payload)
}

#[tauri::command]
pub async fn download_release_apk_cmd(
    app: AppHandle,
    client: tauri::State<'_, reqwest::Client>,
) -> Result<ApkDownloadResult, String> {
    let payload = fetch_latest_release_payload(&client).await?;
    let release_info = build_latest_release_info(payload)?;
    let asset = release_info
        .apk_asset
        .ok_or_else(|| "Latest release does not contain an installable APK asset.".to_string())?;
    let download_path = build_download_path(&app, &asset.name)?;

    prepare_download_path(&download_path).await?;

    let response = client
        .get(&asset.download_url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", "Simple-live")
        .send()
        .await
        .map_err(|error| format!("Failed to start APK download: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("Failed to download APK: HTTP {}", response.status()));
    }

    let mut file = fs::File::create(&download_path)
        .await
        .map_err(|error| format!("Failed to create APK file: {error}"))?;
    let mut stream = response.bytes_stream();

    while let Some(next_chunk) = stream.next().await {
        let chunk = next_chunk.map_err(|error| format!("Failed while downloading APK: {error}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Failed to write APK file: {error}"))?;
    }

    file.flush()
        .await
        .map_err(|error| format!("Failed to flush APK file: {error}"))?;

    Ok(ApkDownloadResult {
        version: release_info.latest_version,
        file_name: asset.name,
        file_path: download_path.to_string_lossy().into_owned(),
    })
}
