use std::collections::HashMap;
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
    resources: Arc<Mutex<HashMap<String, Url>>>,
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

    fn register_resource(&self, base: &Url, path: &str) -> Result<String, actix_web::Error> {
        let url = base.join(path).map_err(|_| actix_web::error::ErrorBadGateway("Upstream request failed"))?;
        let root = Url::parse(&self.source.url).map_err(|_| actix_web::error::ErrorBadGateway("Invalid stream"))?;
        if crate::network_policy::stream_platform(&url).is_none()
            || crate::network_policy::stream_platform(&url) != crate::network_policy::stream_platform(&root) {
            return Err(actix_web::error::ErrorBadGateway("Unsupported HLS resource scheme"));
        }
        let mut resources = self.resources.lock().map_err(|error| actix_web::error::ErrorInternalServerError(error.to_string()))?;
        if resources.len() >= 4096 { resources.clear(); }
        let key = resources.iter().find(|(_, existing)| *existing == &url)
            .map(|(key, _)| key.clone()).unwrap_or_else(|| crate::proxy::session_id());
        resources.insert(key.clone(), url);
        Ok(format!("/resource?session={}&resource={key}", self.id))
    }

    fn rewrite_manifest(&self, base: &Url, manifest: &str) -> Result<String, actix_web::Error> {
        let expression = regex::Regex::new(r#"URI="([^"]+)""#)
            .map_err(actix_web::error::ErrorInternalServerError)?;
        let mut lines = Vec::new();
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { lines.push(String::new()); continue; }
            if !trimmed.starts_with('#') {
                lines.push(self.register_resource(base, trimmed)?);
                continue;
            }
            let mut rewritten = line.to_string();
            for capture in expression.captures_iter(line) {
                let path = self.register_resource(base, &capture[1])?;
                rewritten = rewritten.replace(&capture[0], &format!("URI=\"{path}\""));
            }
            lines.push(rewritten);
        }
        Ok(format!("{}\n", lines.join("\n")))
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
            .get(resource).cloned().ok_or_else(|| actix_web::error::ErrorNotFound("Unknown HLS resource"))?
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
