use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use once_cell::sync::Lazy;
use reqwest::{RequestBuilder, Response};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static LIMITS: Lazy<HashMap<&'static str, Arc<Semaphore>>> = Lazy::new(|| {
    ["douyu", "douyin", "huya", "bilibili", "other"]
        .into_iter().map(|name| (name, Arc::new(Semaphore::new(2)))).collect()
});

/// Holds the platform slot until the API response body is consumed or discarded.
pub struct PlatformResponse {
    response: Response,
    _permit: OwnedSemaphorePermit,
}

impl Deref for PlatformResponse {
    type Target = Response;
    fn deref(&self) -> &Response { &self.response }
}

impl PlatformResponse {
    pub fn error_for_status(self) -> reqwest::Result<Self> {
        self.response.error_for_status_ref()?;
        Ok(self)
    }

    pub async fn text(self) -> reqwest::Result<String> {
        let Self { response, _permit } = self;
        response.text().await
    }

    pub async fn json<T: serde::de::DeserializeOwned>(self) -> reqwest::Result<T> {
        let Self { response, _permit } = self;
        response.json().await
    }

}

pub trait LimitedRequest {
    fn send_limited(self) -> impl Future<Output = reqwest::Result<PlatformResponse>> + Send;
}

impl LimitedRequest for RequestBuilder {
    fn send_limited(self) -> impl Future<Output = reqwest::Result<PlatformResponse>> + Send {
        async move {
            let (client, request) = self.build_split();
            let mut request = request?;
            *request.timeout_mut() = Some(Duration::from_secs(20));
            let platform = [
                ("douyu", &["douyu.com", "douyucdn.cn"][..]),
                ("douyin", &["douyin.com", "amemv.com"][..]),
                ("huya", &["huya.com"][..]),
                ("bilibili", &["bilibili.com"][..]),
            ].into_iter().find_map(|(name, domains)| {
                crate::network_policy::has_domain(request.url(), domains).then_some(name)
            }).unwrap_or("other");
            // These process-lifetime semaphores are never closed.
            let permit = LIMITS[platform].clone().acquire_owned().await
                .expect("Platform request semaphore must remain open");
            let mut attempts = 0;
            loop {
                let Some(copy) = request.try_clone() else {
                    return client.execute(request).await.map(|response| PlatformResponse { response, _permit: permit });
                };
                match client.execute(copy).await {
                    Ok(response) => return Ok(PlatformResponse { response, _permit: permit }),
                    Err(error) if attempts < 2 && (error.is_connect() || error.is_timeout()) => {
                        tokio::time::sleep(Duration::from_millis(500 * (1 << attempts))).await;
                        attempts += 1;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }
}
