use md5::Digest;
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    redirect::Policy,
    Client,
};
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

const DOUYU_SEARCH_REFERER: &str = "https://www.douyu.com/search/";
const DOUYU_SEARCH_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";
const DOUYU_SEARCH_PAGE_SIZE: usize = 20;
const DOUYU_SEARCH_ROOM_API: &str = "https://www.douyu.com/japi/search/api/searchShow";
const DOUYU_SEARCH_ANCHOR_API: &str = "https://www.douyu.com/japi/search/api/searchUser";

#[derive(Debug, Clone, Serialize)]
pub struct DouyuSearchResultItem {
    pub room_id: String,
    pub user_name: String,
    pub room_title: Option<String>,
    pub avatar: Option<String>,
    pub live_status: bool,
    pub category: Option<String>,
    pub fans_count: Option<String>,
}

pub async fn perform_anchor_search(
    keyword: &str,
) -> Result<Vec<DouyuSearchResultItem>, Box<dyn std::error::Error>> {
    let client = build_search_client()?;
    let did = build_search_did()?;
    let encoded_keyword = percent_encode(keyword.as_bytes(), NON_ALPHANUMERIC).to_string();

    let room_results = fetch_search_room_results(&client, &did, &encoded_keyword).await?;
    let anchor_results = fetch_search_anchor_results(&client, &did, &encoded_keyword).await?;

    let mut merged_results = room_results;
    for anchor_result in anchor_results {
        if merged_results
            .iter()
            .any(|item| item.room_id == anchor_result.room_id)
        {
            continue;
        }
        merged_results.push(anchor_result);
    }

    Ok(merged_results)
}

fn build_search_client() -> Result<Client, reqwest::Error> {
    let mut default_headers = HeaderMap::new();
    default_headers.insert("User-Agent", HeaderValue::from_static(DOUYU_SEARCH_USER_AGENT));

    Client::builder()
        .redirect(Policy::limited(10))
        .no_proxy()
        .default_headers(default_headers)
        .build()
}

fn build_search_did() -> Result<String, Box<dyn std::error::Error>> {
    let mut hasher = md5::Md5::new();
    hasher.update(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            .to_string(),
    );
    Ok(format!("{:x}", hasher.finalize()))
}

async fn fetch_search_room_results(
    client: &Client,
    did: &str,
    encoded_keyword: &str,
) -> Result<Vec<DouyuSearchResultItem>, Box<dyn std::error::Error>> {
    let url = format!(
        "{DOUYU_SEARCH_ROOM_API}?kw={encoded_keyword}&page=1&pageSize={DOUYU_SEARCH_PAGE_SIZE}"
    );
    let json = send_search_request(client, did, &url).await?;
    let items = json
        .get("data")
        .and_then(|item| item.get("relateShow"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(items
        .iter()
        .filter_map(map_room_search_item)
        .collect::<Vec<DouyuSearchResultItem>>())
}

async fn fetch_search_anchor_results(
    client: &Client,
    did: &str,
    encoded_keyword: &str,
) -> Result<Vec<DouyuSearchResultItem>, Box<dyn std::error::Error>> {
    let url = format!(
        "{DOUYU_SEARCH_ANCHOR_API}?kw={encoded_keyword}&page=1&pageSize={DOUYU_SEARCH_PAGE_SIZE}&filterType=1"
    );
    let json = send_search_request(client, did, &url).await?;
    let items = json
        .get("data")
        .and_then(|item| item.get("relateUser"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(items
        .iter()
        .filter_map(map_anchor_search_item)
        .collect::<Vec<DouyuSearchResultItem>>())
}

async fn send_search_request(
    client: &Client,
    did: &str,
    url: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let json = client
        .get(url)
        .header("Referer", DOUYU_SEARCH_REFERER)
        .header("Cookie", format!("dy_did={did}; acf_did={did}"))
        .send()
        .await?
        .json::<Value>()
        .await?;

    let error_code = json.get("error").and_then(value_to_i64).unwrap_or(-1);
    if error_code != 0 {
        let message = json
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or("Douyu search failed");
        return Err(format!("Douyu search error {error_code}: {message}").into());
    }

    Ok(json)
}

fn map_room_search_item(item: &Value) -> Option<DouyuSearchResultItem> {
    let room_id = item.get("rid").and_then(value_to_string)?;
    let room_title = item
        .get("roomName")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let user_name = item
        .get("nickName")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "斗鱼主播".to_string());

    Some(DouyuSearchResultItem {
        room_id,
        user_name,
        room_title,
        avatar: item
            .get("roomSrc")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        live_status: true,
        category: item
            .get("cateName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        fans_count: item
            .get("hot")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn map_anchor_search_item(item: &Value) -> Option<DouyuSearchResultItem> {
    let anchor_info = item.get("anchorInfo")?;
    let room_id = anchor_info.get("rid").and_then(value_to_string)?;
    let is_live = anchor_info.get("isLive").and_then(value_to_i64).unwrap_or(0) == 1;
    let is_loop = anchor_info
        .get("videoLoop")
        .and_then(value_to_i64)
        .unwrap_or(0)
        == 1;

    Some(DouyuSearchResultItem {
        room_id,
        user_name: anchor_info
            .get("nickName")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "斗鱼主播".to_string()),
        room_title: anchor_info
            .get("roomName")
            .or_else(|| anchor_info.get("description"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        avatar: anchor_info
            .get("avatar")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        live_status: is_live && !is_loop,
        category: anchor_info
            .get("cateName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        fans_count: anchor_info
            .get("fansNumStr")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.to_string()),
        _ => None,
    }
}
