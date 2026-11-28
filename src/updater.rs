//! 检查 GitHub Releases 是否有新版本。

use std::time::Duration;

use serde_json::Value;

const LATEST_URL: &str = "https://api.github.com/repos/ACG-j/GameHaven/releases/latest";

pub struct Release {
    pub tag: String,
}

pub fn latest() -> Result<Option<Release>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut response = match agent
        .get(LATEST_URL)
        .header("User-Agent", "GameHaven")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(format!("检查更新失败：{error}")),
    };
    let value: Value = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json()
        .map_err(|error| format!("无法解析更新信息：{error}"))?;
    let tag = value["tag_name"].as_str().unwrap_or("").trim().to_owned();
    if tag.is_empty() {
        return Ok(None);
    }
    Ok(Some(Release { tag }))
}

/// 比较版本号，返回提示文案。
pub fn compare(current: &str, latest: &str) -> String {
    let current = current.trim_start_matches('v');
    let latest_tag = latest.trim_start_matches('v');
    if latest_tag == current {
        format!("已是最新版本（{current}）。")
    } else {
        format!("发现新版本 {latest}（当前 {current}）。")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(compare("0.1.0", "v0.1.0").contains("最新"));
        assert!(compare("0.1.0", "v0.2.0").contains("0.2.0"));
    }
}
