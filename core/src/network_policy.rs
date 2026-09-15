use reqwest::Url;

/// Exact DNS suffix matching; URLs cannot select arbitrary hosts or local ports.
pub(crate) fn has_domain(url: &Url, domains: &[&str]) -> bool {
    let Some(host) = url.host_str() else { return false; };
    matches!(url.scheme(), "https" | "http")
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.port(), None | Some(80) | Some(443))
        && domains.iter().any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

pub(crate) fn stream_platform(url: &Url) -> Option<&'static str> {
    [
        ("bilibili", &["bilivideo.com", "bilivideo.cn", "biliapi.net"][..]),
        ("douyu", &["douyucdn.cn", "douyucdn2.cn", "douyucdn3.cn", "douyucdn4.cn", "douyucdn5.cn", "douyucdn6.cn", "douyucdn7.cn", "douyucdn8.cn", "douyucdn9.cn", "douyucdn10.cn", "douyucdn11.cn", "douyucdn12.cn", "douyucdn13.cn"][..]),
        ("huya", &["huya.com", "hycdn.cn", "huyahttpdns.com"][..]),
        ("douyin", &["douyincdn.com", "douyincdn.cn", "douyin.com", "bytecdn.cn", "ibytedtos.com", "bytefcdnrd.com"][..]),
    ].into_iter().find_map(|(platform, domains)| has_domain(url, domains).then_some(platform))
}

pub(crate) fn image_platform(url: &Url) -> Option<&'static str> {
    [
        ("bilibili", &["hdslb.com", "bilibili.com"][..]),
        ("douyu", &["douyucdn.cn", "douyucdn2.cn", "douyu.com", "douyucdn.cn"][..]),
        ("huya", &["huya.com", "huyaimg.com", "msstatic.com"][..]),
        ("douyin", &["douyinpic.com", "douyin.com", "byteimg.com", "douyincdn.com", "ibytedtos.com"][..]),
    ].into_iter().find_map(|(platform, domains)| has_domain(url, domains).then_some(platform))
}

/// Redirects must stay on the same host; manually supplied Cookie headers never migrate.
pub(crate) fn redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 3
            || !attempt.previous().first().is_some_and(|first| {
                first.host_str() == attempt.url().host_str()
                    && (first.scheme() != "https" || attempt.url().scheme() == "https")
                    && matches!(attempt.url().port(), None | Some(80) | Some(443))
            })
        {
            attempt.stop()
        } else {
            attempt.follow()
        }
    })
}
