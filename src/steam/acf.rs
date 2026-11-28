//! 生成或重写 `appmanifest_<appid>.acf`，供 Steam 识别安装记录。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct GameInfo<'a> {
    pub app_id: u32,
    pub name: &'a str,
    pub installdir: &'a str,
    pub build_id: &'a str,
    pub size_on_disk: u64,
    /// `(depot_id, manifest_id)`
    pub depots: &'a [(u32, u64)],
}

/// 把游戏名清洗成合法的安装目录名。
pub fn sanitize_installdir(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "Game".to_owned()
    } else {
        cleaned.to_owned()
    }
}

pub fn path(steamapps: &Path, app_id: u32) -> PathBuf {
    steamapps.join(format!("appmanifest_{app_id}.acf"))
}

pub fn write(steamapps: &Path, game: &GameInfo<'_>) -> Result<PathBuf, String> {
    let target = path(steamapps, game.app_id);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
    }
    if target.exists() {
        let backup = target.with_extension("acf.bak");
        fs::copy(&target, &backup)
            .map_err(|error| format!("无法备份 {}：{error}", backup.display()))?;
    }
    fs::write(&target, render(game))
        .map_err(|error| format!("无法写入 {}：{error}", target.display()))?;
    Ok(target)
}

fn render(game: &GameInfo<'_>) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut installed = String::new();
    for (depot, manifest) in game.depots {
        installed.push_str(&format!(
            "\t\t\"{depot}\"\n\t\t{{\n\t\t\t\"manifest\"\t\t\"{manifest}\"\n\t\t\t\"size\"\t\t\"0\"\n\t\t}}\n"
        ));
    }
    format!(
        "\"AppState\"\n\
{{\n\
\t\"appid\"\t\t\"{app_id}\"\n\
\t\"Universe\"\t\t\"1\"\n\
\t\"name\"\t\t\"{name}\"\n\
\t\"StateFlags\"\t\t\"4\"\n\
\t\"installdir\"\t\t\"{installdir}\"\n\
\t\"LastUpdated\"\t\t\"{now}\"\n\
\t\"SizeOnDisk\"\t\t\"{size}\"\n\
\t\"StagingSize\"\t\t\"0\"\n\
\t\"buildid\"\t\t\"{build}\"\n\
\t\"LastOwner\"\t\t\"0\"\n\
\t\"UpdateResult\"\t\t\"0\"\n\
\t\"BytesToDownload\"\t\t\"0\"\n\
\t\"BytesDownloaded\"\t\t\"0\"\n\
\t\"BytesToStage\"\t\t\"0\"\n\
\t\"BytesStaged\"\t\t\"0\"\n\
\t\"TargetBuildID\"\t\t\"{build}\"\n\
\t\"AutoUpdateBehavior\"\t\t\"0\"\n\
\t\"AllowOtherDownloadsWhileRunning\"\t\t\"0\"\n\
\t\"ScheduledAutoUpdate\"\t\t\"0\"\n\
\t\"DownloadType\"\t\t\"1\"\n\
\t\"InstalledDepots\"\n\
\t{{\n\
{installed}\
\t}}\n\
\t\"UserConfig\"\n\
\t{{\n\
\t\t\"language\"\t\t\"english\"\n\
\t}}\n\
\t\"MountedConfig\"\n\
\t{{\n\
\t\t\"language\"\t\t\"english\"\n\
\t}}\n\
}}\n",
        app_id = game.app_id,
        name = game.name,
        installdir = game.installdir,
        size = game.size_on_disk,
        build = game.build_id,
        installed = installed,
        now = now,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_installdir() {
        assert_eq!(sanitize_installdir("Forza Horizon 6"), "Forza Horizon 6");
        assert_eq!(sanitize_installdir("a/b:c*?"), "a_b_c__");
        assert_eq!(sanitize_installdir("  ..  "), "Game");
    }

    #[test]
    fn writes_manifest_with_installed_depots() {
        let directory = tempfile::tempdir().unwrap();
        let steamapps = directory.path();
        let path = write(
            steamapps,
            &GameInfo {
                app_id: 2483190,
                name: "Forza Horizon 6",
                installdir: "ForzaHorizon6",
                build_id: "25588841",
                size_on_disk: 1234,
                depots: &[(2483191, 3954142684088109524)],
            },
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"appid\"\t\t\"2483190\""));
        assert!(text.contains("\"installdir\"\t\t\"ForzaHorizon6\""));
        assert!(text.contains("\"manifest\"\t\t\"3954142684088109524\""));
        assert!(text.contains("\"SizeOnDisk\"\t\t\"1234\""));
    }
}
