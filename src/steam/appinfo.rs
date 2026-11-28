//! 从公开的 Steam PICS 镜像（api.steamcmd.net）读取 appinfo：
//! 名称、buildid、depot 列表和最新 manifest，以及 DLC AppID。

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

const INFO_URL: &str = "https://api.steamcmd.net/v1/info";

#[derive(Clone, Debug, Default)]
pub struct AppInfo {
    pub name: String,
    pub build_id: String,
    /// `(depot_id, latest_manifest_id)`
    pub depots: Vec<(u32, u64)>,
    pub dlcs: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct InfoResponse {
    #[serde(default)]
    data: HashMap<String, AppData>,
}

#[derive(Debug, Deserialize)]
struct AppData {
    #[serde(default)]
    common: Option<Common>,
    #[serde(default)]
    depots: Option<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize)]
struct Common {
    #[serde(default)]
    name: String,
}

pub fn fetch(app_id: u32) -> Result<AppInfo, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    let mut response = agent
        .get(&format!("{INFO_URL}/{app_id}"))
        .call()
        .map_err(|error| format!("appinfo 请求失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "appinfo 返回 HTTP {}。",
            response.status().as_u16()
        ));
    }
    let parsed: InfoResponse = response
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_json()
        .map_err(|error| format!("无法解析 appinfo：{error}"))?;
    parse(parsed, app_id)
}

fn parse(parsed: InfoResponse, app_id: u32) -> Result<AppInfo, String> {
    let app = parsed
        .data
        .get(&app_id.to_string())
        .ok_or_else(|| format!("appinfo 里没有 AppID {app_id}。"))?;
    let name = app
        .common
        .as_ref()
        .map(|common| common.name.clone())
        .unwrap_or_default();

    let mut info = AppInfo {
        name,
        ..AppInfo::default()
    };
    let Some(depots) = &app.depots else {
        return Ok(info);
    };
    if let Some(build) = depots
        .get("branches")
        .and_then(|value| value.get("public"))
        .and_then(|value| value.get("buildid"))
        .and_then(Value::as_str)
    {
        info.build_id = build.to_owned();
    }
    for (key, value) in depots {
        let Ok(depot_id) = key.parse::<u32>() else {
            continue;
        };
        if let Some(gid) = value
            .pointer("/manifests/public/gid")
            .and_then(Value::as_str)
            && let Ok(manifest_id) = gid.parse::<u64>()
        {
            info.depots.push((depot_id, manifest_id));
        }
        if let Some(dlc) = value.get("dlcappid").and_then(Value::as_str)
            && let Ok(dlc_id) = dlc.parse::<u32>()
        {
            info.dlcs.push(dlc_id);
        }
    }
    info.depots.sort();
    info.dlcs.sort();
    info.dlcs.dedup();
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "data": {
            "2483190": {
                "common": {"name": "Forza Horizon 6"},
                "depots": {
                    "branches": {"public": {"buildid": "26422872"}},
                    "2483191": {"manifests": {"public": {"gid": "8260642490601717688"}}},
                    "4439240": {"dlcappid": "4439240", "manifests": {"public": {"gid": "111"}}}
                }
            }
        }
    }"#;

    #[test]
    fn parses_depots_manifests_and_dlcs() {
        let parsed: InfoResponse = serde_json::from_str(SAMPLE).unwrap();
        let info = parse(parsed, 2483190).unwrap();
        assert_eq!(info.name, "Forza Horizon 6");
        assert_eq!(info.build_id, "26422872");
        assert_eq!(
            info.depots,
            vec![(2483191, 8260642490601717688), (4439240, 111)]
        );
        assert_eq!(info.dlcs, vec![4439240]);
    }
}
