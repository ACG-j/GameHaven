use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

const APP_DETAILS_URL: &str = "https://store.steampowered.com/api/appdetails";
const STORE_SEARCH_URL: &str = "https://store.steampowered.com/api/storesearch";

#[derive(Debug, Deserialize)]
pub struct StoreApp {
    steam_appid: u32,
    pub name: String,
    pub platforms: Option<Platforms>,
    /// 商店资料里列出的 DLC AppID。
    #[serde(default)]
    pub dlc: Vec<u32>,
}

/// 商店搜索结果里的一项。
#[derive(Debug, Deserialize)]
pub struct SearchItem {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
pub struct Platforms {
    pub windows: bool,
    pub mac: bool,
    pub linux: bool,
}

#[derive(Deserialize)]
struct Entry {
    success: bool,
    data: Option<StoreApp>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .max_redirects(0)
        .build()
        .into()
}

pub fn lookup(app_id: u32) -> Result<Option<StoreApp>, String> {
    let agent = agent();
    let mut response = agent
        .get(APP_DETAILS_URL)
        .query("appids", app_id.to_string())
        .query("l", "schinese")
        .call()
        .map_err(request_error)?;
    if !response.status().is_success() {
        return Err(request_error(ureq::Error::StatusCode(
            response.status().as_u16(),
        )));
    }

    let mut entries: HashMap<String, Entry> = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_json()
        .map_err(request_error)?;
    match entries.remove(&app_id.to_string()) {
        Some(Entry { success: false, .. }) => Ok(None),
        Some(Entry {
            success: true,
            data: Some(app),
        }) if app.steam_appid == app_id && !app.name.trim().is_empty() => Ok(Some(app)),
        _ => Err("Steam 商店返回的 AppID 或游戏信息有误。".into()),
    }
}

/// 按名称搜索 Steam 商店，返回 AppID 和名称。
pub fn search(term: &str) -> Result<Vec<SearchItem>, String> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(Vec::new());
    }
    let agent = agent();
    let mut response = agent
        .get(STORE_SEARCH_URL)
        .query("term", term)
        .query("l", "schinese")
        .query("cc", "CN")
        .call()
        .map_err(request_error)?;
    if !response.status().is_success() {
        return Err(request_error(ureq::Error::StatusCode(
            response.status().as_u16(),
        )));
    }
    let result: SearchResponse = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_json()
        .map_err(request_error)?;
    Ok(result
        .items
        .into_iter()
        .filter(|item| item.id > 0 && !item.name.trim().is_empty())
        .collect())
}

fn request_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => "Steam 商店查询超时，请重试。".into(),
        ureq::Error::StatusCode(429) => "Steam 商店请求过于频繁，请稍后重试。".into(),
        ureq::Error::StatusCode(code) => format!("Steam 商店返回 HTTP {code}，请重试。"),
        ureq::Error::Json(_) | ureq::Error::BodyExceedsLimit(_) => {
            "Steam 商店返回了无法读取的游戏信息。".into()
        }
        _ => "无法连接 Steam 商店，请检查网络后重试。".into(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dlc_and_platforms() {
        let json = r#"{
            "440": {
                "success": true,
                "data": {
                    "steam_appid": 440,
                    "name": "Team Fortress 2",
                    "platforms": {"windows": true, "mac": true, "linux": true},
                    "dlc": [411, 412]
                }
            }
        }"#;
        let mut entries: HashMap<String, Entry> = serde_json::from_str(json).unwrap();
        let app = entries.remove("440").unwrap().data.unwrap();
        assert_eq!(app.name, "Team Fortress 2");
        assert_eq!(app.dlc, vec![411, 412]);
    }

    #[test]
    fn parses_store_search_items() {
        let json = r#"{"items":[{"id":440,"name":"Team Fortress 2","tiny_image":"x"},{"id":0,"name":""}]}"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        let items: Vec<_> = response
            .items
            .into_iter()
            .filter(|item| item.id > 0 && !item.name.trim().is_empty())
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 440);
    }
}
