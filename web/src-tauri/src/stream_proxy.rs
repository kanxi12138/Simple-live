use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};

use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::TryStreamExt;
use reqwest::{Client, Url};
use serde::Deserialize;

use crate::StreamSource;

const MANIFEST_CONTENT_TYPE: &str = "application/vnd.apple.mpegurl";

/// An immutable playback context; HLS resources share only this stream's headers.
#[derive(Clone)]
pub(crate) struct StreamSession {
    source: StreamSource,
    id: String,
    resources: Arc<Mutex<ResourceRegistry>>,
}

impl StreamSession {
    /// Captures the validated source so later room changes cannot retarget old requests.
    pub(crate) fn new(source: StreamSource) -> Self {
        Self { source, id: crate::proxy::session_id(), resources: Arc::default() }
    }

    /// Returns the root URL path, including a session identifier to reject stale players.
    pub(crate) fn root_path(&self) -> String {
        let path = if self.source.format == "hls" { "live.m3u8" } else { "live.flv" };
        format!("/{path}?session={}", self.id)
    }

    fn rewrite_manifest(&self, base: &Url, manifest: &str) -> Result<String, actix_web::Error> {
        let expression = regex::Regex::new(r#"URI="([^"]+)""#)
            .map_err(actix_web::error::ErrorInternalServerError)?;
        let root = Url::parse(&self.source.url).map_err(|_| actix_web::error::ErrorBadGateway("Invalid stream"))?;
        let mut urls = Vec::new();
        let mut resolve = |path: &str| -> Result<(), actix_web::Error> {
            let url = base.join(path).map_err(|_| actix_web::error::ErrorBadGateway("Invalid HLS resource"))?;
            if crate::network_policy::stream_platform(&url).is_none()
                || crate::network_policy::stream_platform(&url) != crate::network_policy::stream_platform(&root) {
                return Err(actix_web::error::ErrorBadGateway("Unsupported HLS resource"));
            }
            urls.push(url);
            Ok(())
        };
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            if !trimmed.starts_with('#') { resolve(trimmed)?; }
            else { for capture in expression.captures_iter(line) { resolve(&capture[1])?; } }
        }
        let keys = self.resources.lock().map_err(|_| actix_web::error::ErrorInternalServerError("HLS state unavailable"))?
            .update(base, &urls, Instant::now()).map_err(actix_web::error::ErrorBadGateway)?;
        let mut keys = keys.into_iter();
        let mut path = || format!("/resource?session={}&resource={}", self.id, keys.next().unwrap_or_default());
        let mut lines = Vec::new();
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { lines.push(String::new()); }
            else if !trimmed.starts_with('#') { lines.push(path()); }
            else {
                lines.push(expression.replace_all(line, |_: &regex::Captures<'_>| format!("URI=\"{}\"", path())).into_owned());
            }
        }
        Ok(format!("{}\n", lines.join("\n")))
    }

}

#[derive(Clone)]
struct ResourceEntry {
    url: Url,
    last_used: Instant,
    detached_at: Option<Instant>,
}

#[derive(Clone, Default)]
struct ResourceRegistry {
    root: Option<Url>,
    entries: HashMap<String, ResourceEntry>,
    by_url: HashMap<Url, String>,
    playlists: HashMap<Url, HashSet<Url>>,
}

impl ResourceRegistry {
    // Stage the entire update so a capacity error cannot invalidate a published playlist.
    fn update(&mut self, base: &Url, urls: &[Url], now: Instant) -> Result<Vec<String>, &'static str> {
        let mut staged = self.clone();
        staged.root.get_or_insert_with(|| base.clone());
        staged.playlists.insert(base.clone(), urls.iter().cloned().collect());
        let mut referenced = HashSet::new();
        let mut pending: Vec<_> = staged.root.iter().cloned().collect();
        while let Some(url) = pending.pop() {
            if !referenced.insert(url.clone()) { continue; }
            if let Some(children) = staged.playlists.get(&url) { pending.extend(children.iter().cloned()); }
        }
        for entry in staged.entries.values_mut() {
            if referenced.contains(&entry.url) { entry.detached_at = None; }
            else if entry.detached_at.is_none() { entry.detached_at = Some(now); }
        }
        let missing: HashSet<_> = urls.iter().filter(|url| !staged.by_url.contains_key(*url)).collect();
        let needed = (staged.entries.len() + missing.len()).saturating_sub(4096);
        let mut expired: Vec<_> = staged.entries.iter().filter(|(_, entry)| {
            entry.detached_at.is_some_and(|time| now.saturating_duration_since(time) >= Duration::from_secs(120))
        }).map(|(key, entry)| (key.clone(), entry.last_used)).collect();
        expired.sort_by_key(|(_, time)| *time);
        if expired.len() < needed { return Err("HLS active resource capacity exceeded"); }
        for (key, _) in expired.into_iter().take(needed) {
            if let Some(entry) = staged.entries.remove(&key) {
                staged.by_url.remove(&entry.url);
                staged.playlists.remove(&entry.url);
            }
        }
        let mut keys = Vec::new();
        for url in urls {
            let key = staged.by_url.entry(url.clone()).or_insert_with(crate::proxy::session_id).clone();
            staged.entries.entry(key.clone()).and_modify(|entry| entry.last_used = now)
                .or_insert(ResourceEntry { url: url.clone(), last_used: now, detached_at: None });
            keys.push(key);
        }
        *self = staged;
        Ok(keys)
    }

    fn get(&mut self, key: &str) -> Option<Url> {
        self.entries.get_mut(key).map(|entry| { entry.last_used = Instant::now(); entry.url.clone() })
    }
}

/// Session and registered resource identifiers supplied by the player.
#[derive(Deserialize)]
pub(crate) struct StreamQuery {
    session: Option<String>,
    resource: Option<String>,
}

/// Forwards FLV or HLS resources using the captured request headers and range.
/// Returns upstream HTTP failures and rejects resources from another session.
pub(crate) async fn forward_stream(
    request: HttpRequest,
    query: web::Query<StreamQuery>,
    session: web::Data<StreamSession>,
    client: web::Data<Client>,
) -> Result<HttpResponse, actix_web::Error> {
    if query.session.as_deref() != Some(session.id.as_str()) {
        return Ok(HttpResponse::Gone().finish());
    }
    let url = if let Some(resource) = &query.resource {
        session.resources.lock().map_err(|error| actix_web::error::ErrorInternalServerError(error.to_string()))?
            .get(resource).ok_or_else(|| actix_web::error::ErrorNotFound("Unknown HLS resource"))?
    } else {
        Url::parse(&session.source.url).map_err(|_| actix_web::error::ErrorBadGateway("Upstream request failed"))?
    };
    let mut upstream = client.get(url).headers(session.source.headers.clone());
    if let Some(range) = request.headers().get("Range") {
        upstream = upstream.header("Range", range.to_str().map_err(actix_web::error::ErrorBadRequest)?);
    }
    let mut response = upstream.send().await.map_err(|_| actix_web::error::ErrorBadGateway("Upstream request failed"))?;
    let status = actix_web::http::StatusCode::from_u16(response.status().as_u16())
        .map_err(|_| actix_web::error::ErrorBadGateway("Upstream request failed"))?;
    if !status.is_success() { return Ok(HttpResponse::BadGateway().finish()); }
    let content_type = response.headers().get("Content-Type").and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream").to_string();
    let is_manifest = response.url().path().ends_with(".m3u8")
        || content_type.to_ascii_lowercase().contains("mpegurl")
        || (query.resource.is_none() && session.source.format == "hls");
    if is_manifest {
        let base = response.url().clone();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| actix_web::error::ErrorBadGateway("Playlist interrupted"))? {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(actix_web::error::ErrorBadGateway("Playlist too large"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let manifest = String::from_utf8(bytes).map_err(|_| actix_web::error::ErrorBadGateway("Invalid playlist encoding"))?;
        if manifest.lines().any(|line| line.starts_with("#EXT-X-KEY:") && !line.contains("METHOD=NONE")) {
            return Err(actix_web::error::ErrorForbidden("Encrypted playback is not supported"));
        }
        if manifest.len() > 1024 * 1024 || !manifest.trim_start().starts_with("#EXTM3U") {
            return Err(actix_web::error::ErrorBadGateway("Invalid HLS playlist"));
        }
        return Ok(HttpResponse::Ok().content_type(MANIFEST_CONTENT_TYPE)
            .insert_header(("Cache-Control", "no-store"))
            .body(session.rewrite_manifest(&base, &manifest)?));
    }
    let mut builder = HttpResponse::build(status);
    builder.content_type(content_type).insert_header(("Cache-Control", "no-store"));
    for header in ["Content-Range", "Content-Length", "Accept-Ranges"] {
        if let Some(value) = response.headers().get(header).and_then(|value| value.to_str().ok()) {
            builder.insert_header((header, value));
        }
    }
    Ok(builder.streaming(response.bytes_stream().map_err(|_| actix_web::error::ErrorBadGateway("Stream interrupted"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_playlists_keep_current_ids_and_evict_only_expired_resources() {
        let root = Url::parse("https://example.bilivideo.com/live.m3u8").unwrap();
        let mut registry = ResourceRegistry::default();
        let start = Instant::now();
        let mut previous: Option<String> = None;
        for index in 0..4200 {
            let url = root.join(&format!("{index}.ts")).unwrap();
            let now = start + Duration::from_secs(index * 3);
            let key = registry.update(&root, &[url.clone()], now).unwrap()[0].clone();
            assert_eq!(registry.update(&root, &[url.clone()], now).unwrap()[0], key);
            assert_eq!(registry.get(&key), Some(url));
            if let Some(old) = previous { assert!(registry.get(&old).is_some()); }
            previous = Some(key);
            assert!(registry.entries.len() <= 4096);
        }
    }

    #[test]
    fn protected_capacity_failure_is_transactional() {
        let root = Url::parse("https://example.bilivideo.com/live.m3u8").unwrap();
        let mut registry = ResourceRegistry::default();
        let urls: Vec<_> = (0..4096).map(|index| root.join(&format!("{index}.ts")).unwrap()).collect();
        let now = Instant::now();
        let keys = registry.update(&root, &urls, now).unwrap();
        assert!(registry.update(&root, &[root.join("new.ts").unwrap()], now).is_err());
        assert_eq!(registry.playlists[&root].len(), 4096);
        assert!(keys.iter().all(|key| registry.get(key).is_some()));
        registry.update(&root, &urls[1..], now).unwrap();
        assert!(registry.update(&root, &[root.join("new.ts").unwrap()], now + Duration::from_secs(119)).is_err());
        assert!(registry.update(&root, &[root.join("new.ts").unwrap()], now + Duration::from_secs(120)).is_ok());
    }

    #[test]
    fn invalid_manifest_does_not_register_partial_resources() {
        let source = StreamSource { url: "https://example.bilivideo.com/live.m3u8".into(), format: "hls".into(), ..Default::default() };
        let session = StreamSession::new(source);
        let base = Url::parse(&session.source.url).unwrap();
        assert!(session.rewrite_manifest(&base, "#EXTM3U\ngood.ts\nhttps://invalid.example/bad.ts").is_err());
        assert!(session.resources.lock().unwrap().entries.is_empty());
    }
}
