// Adapted from dart_simple_live ba828e6783b176ea5709fcd09f0eb01dfaceeb51 (GPL-3.0).
// Copyright xiaoyaocz and contributors; see THIRD_PARTY_NOTICES.md.
// Only signs the AntiCode supplied by the public mobile room page.
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use md5::{Digest, Md5};
pub(super) const PLAY_USER_AGENT: &str = crate::platforms::common::http_client::DEFAULT_USER_AGENT;
const WAP_PLATFORM: i64 = 103;

fn md5_hex(value: &str) -> String {
    format!("{:x}", Md5::digest(value.as_bytes()))
}

/// Signs `token` for the stream and presenter; rejects incomplete AntiCode.
pub(super) fn sign_anti_code(stream: &str, presenter: i64, token: &str) -> Result<String> {
    let parameters: BTreeMap<_, _> = url::form_urlencoded::parse(token.as_bytes())
        .into_owned().collect();
    let Some(encoded_prefix) = parameters.get("fm") else { return Ok(token.to_string()); };
    let ctype = parameters.get("ctype").map(String::as_str).context("虎牙页面 Token 缺少 ctype")?;
    let platform: i64 = parameters.get("t").map(String::as_str).context("虎牙页面 Token 缺少 t")?.parse()?;
    let ws_time = parameters.get("wsTime").context("虎牙 Token 缺少 wsTime")?;
    let fs_value = parameters.get("fs").context("虎牙 Token 缺少 fs")?;
    let decoded = percent_encoding::percent_decode_str(encoded_prefix).decode_utf8()?;
    let prefix = String::from_utf8(STANDARD.decode(decoded.as_bytes())?)?;
    let prefix = prefix.split('_').next().context("虎牙 Token 缺少签名前缀")?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
    let sequence = presenter + timestamp;
    let rotated = (presenter & !0xffff_ffff) | ((presenter as u32).rotate_left(8) as i64);
    let uid = if platform == WAP_PLATFORM { presenter } else { rotated };
    let hash = md5_hex(&format!("{sequence}|{ctype}|{platform}"));
    let secret = md5_hex(&format!("{prefix}_{uid}_{stream}_{hash}_{ws_time}"));
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.extend_pairs([
        ("wsSecret", secret), ("wsTime", ws_time.clone()),
        ("seqid", sequence.to_string()), ("ctype", ctype.to_string()),
        ("ver", "1".to_string()), ("fs", fs_value.clone()),
        ("fm", encoded_prefix.clone()), ("t", platform.to_string()),
    ]);
    if platform == WAP_PLATFORM {
        query.append_pair("uid", &presenter.to_string());
        query.append_pair("uuid", &rand::random::<u32>().to_string());
    } else {
        query.append_pair("u", &rotated.to_string());
    }
    Ok(query.finish())
}
