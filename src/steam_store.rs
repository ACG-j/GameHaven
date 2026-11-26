use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

const APP_DETAILS_URL: &str = "https://store.steampowered.com/api/appdetails";

#[derive(Debug, Deserialize)]
pub struct StoreApp {
    steam_appid: u32,
    pub name: String,
    pub platforms: Option<Platforms>,
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

pub fn lookup(app_id: u32) -> Result<Option<StoreApp>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .max_redirects(0)
        .build()
        .into();
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
