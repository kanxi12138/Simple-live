// Ported from dart_simple_live ba828e6, huya_site.dart and model/tars.
// Copyright xiaoyaocz and contributors. GPL-3.0; see THIRD_PARTY_NOTICES.md.
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use md5::{Digest, Md5};
use tars_stream::prelude::*;

pub(super) const PLAY_USER_AGENT: &str =
    "HYSDK(Windows, 30000002)_APP(pc_exe&7060000&official)_SDK(trans&2.32.3.5646)";
const TOKEN_ENDPOINT: &str = "http://wup.huya.com";
const TOKEN_APP_ID: i32 = 66;
const TUP_VERSION: i16 = 3;
const WAP_PLATFORM: i64 = 103;

struct UserId;

impl StructToTars for UserId {
    fn _encode_to(&self, encoder: &mut TarsEncoder) -> Result<(), EncodeErr> {
        encoder.write_int64(0, 0)?;
        encoder.write_string(1, &String::new())?;
        encoder.write_string(2, &String::new())?;
        encoder.write_string(3, &"pc_exe&7060000&official".to_string())?;
        encoder.write_string(4, &String::new())?;
        encoder.write_int32(5, 0)?;
        encoder.write_string(6, &String::new())?;
        encoder.write_string(7, &String::new())
    }
}

struct TokenRequest<'a>(&'a str);

impl StructToTars for TokenRequest<'_> {
    fn _encode_to(&self, encoder: &mut TarsEncoder) -> Result<(), EncodeErr> {
        encoder.write_string(0, &String::new())?;
        encoder.write_string(1, &self.0.to_string())?;
        encoder.write_int32(2, 0)?;
        encoder.write_struct(3, &UserId)?;
        encoder.write_int32(4, TOKEN_APP_ID)
    }
}

#[derive(Default)]
struct TokenResponse(String);

impl StructFromTars for TokenResponse {
    fn _decode_from(decoder: &mut TarsDecoder) -> Result<Self, DecodeErr> {
        Ok(Self(decoder.read_string(0, true, String::new())?))
    }
}

fn encode_request(stream: &str) -> Result<Vec<u8>> {
    let mut request = TarsEncoder::new();
    request.write_struct(0, &TokenRequest(stream))?;
    let attributes = BTreeMap::from([("tReq".to_string(), request.to_bytes())]);
    let mut payload = TarsEncoder::new();
    payload.write_map(0, &attributes)?;
    let mut packet = TarsEncoder::new();
    packet.write_int16(1, TUP_VERSION)?;
    packet.write_int8(2, 0)?;
    packet.write_int32(3, 0)?;
    packet.write_int32(4, 0)?;
    packet.write_string(5, &"liveui".to_string())?;
    packet.write_string(6, &"getCdnTokenInfoEx".to_string())?;
    packet.write_bytes(7, &payload.to_bytes())?;
    packet.write_int32(8, 0)?;
    packet.write_map(9, &BTreeMap::<String, String>::new())?;
    packet.write_map(10, &BTreeMap::<String, String>::new())?;
    let body = packet.to_bytes();
    let mut framed = ((body.len() + 4) as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(&body);
    Ok(framed)
}

fn decode_token(data: &[u8]) -> Result<String> {
    let length = data.get(..4).context("虎牙 Token 响应缺少长度")?;
    let declared = u32::from_be_bytes(length.try_into()?) as usize;
    if declared != data.len() {
        bail!("虎牙 Token 响应长度不匹配");
    }
    let mut packet = TarsDecoder::from(&data[4..]);
    let buffer = packet.read_bytes(7, true, Default::default())?;
    let mut payload = TarsDecoder::from(buffer.as_ref());
    let attributes = payload.read_map(0, true, BTreeMap::<String, bytes04::Bytes>::new())?;
    if let Some(status) = attributes.get("") {
        let code = TarsDecoder::from(status.as_ref()).read_int32(0, true, 0)?;
        if code != 0 {
            bail!("虎牙 Token 请求失败: {code}");
        }
    }
    let response = attributes.get("tRsp").context("虎牙 Token 响应缺少 tRsp")?;
    let token = TarsDecoder::from(response.as_ref())
        .read_struct(0, true, TokenResponse::default())?.0;
    if token.is_empty() {
        bail!("虎牙 Token 为空");
    }
    Ok(token)
}

/// Requests a fresh FLV token for `stream`; returns protocol or HTTP errors.
pub(super) async fn fetch_token(client: &reqwest::Client, stream: &str) -> Result<String> {
    let response = client.post(TOKEN_ENDPOINT)
        .header("Content-Type", "application/x-wup")
        .header("User-Agent", PLAY_USER_AGENT)
        .header("Origin", "https://m.huya.com/")
        .header("Referer", "https://m.huya.com/")
        .body(encode_request(stream)?).send().await?
        .error_for_status()?.bytes().await?;
    decode_token(&response)
}

fn md5_hex(value: &str) -> String {
    format!("{:x}", Md5::digest(value.as_bytes()))
}

/// Signs `token` for the stream and presenter; rejects incomplete AntiCode.
pub(super) fn sign_anti_code(stream: &str, presenter: i64, token: &str) -> Result<String> {
    let parameters: BTreeMap<_, _> = url::form_urlencoded::parse(token.as_bytes())
        .into_owned().collect();
    let Some(encoded_prefix) = parameters.get("fm") else { return Ok(token.to_string()); };
    let ctype = parameters.get("ctype").map(String::as_str).unwrap_or("huya_pc_exe");
    // The desktop CDN token omits `t`; the reference signs that token with platform 0.
    let platform: i64 = parameters.get("t").map(String::as_str).unwrap_or("0").parse()?;
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
