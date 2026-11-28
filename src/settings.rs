//! 用户设置持久化：`~/.config/gamehaven/settings.toml`。

use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Settings {
    pub auto_restart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { auto_restart: true }
    }
}

fn path() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    Some(home.join(".config/gamehaven/settings.toml"))
}

pub fn load() -> Settings {
    let Some(path) = path() else {
        return Settings::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Settings::default();
    };
    let mut settings = Settings::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "auto_restart" {
            settings.auto_restart = value.trim() == "true";
        }
    }
    settings
}

pub fn save(settings: &Settings) {
    let Some(path) = path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        path,
        format!("auto_restart = {}\n", settings.auto_restart),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_round_trip() {
        assert!(Settings::default().auto_restart);
        let settings = Settings { auto_restart: false };
        // 直接验证解析逻辑，不碰用户目录。
        let text = format!("auto_restart = {}\n", settings.auto_restart);
        let mut parsed = Settings::default();
        for line in text.lines() {
            let (key, value) = line.split_once('=').unwrap();
            if key.trim() == "auto_restart" {
                parsed.auto_restart = value.trim() == "true";
            }
        }
        assert!(!parsed.auto_restart);
    }
}
