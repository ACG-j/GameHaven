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

/// 递归统计目录大小，跳过符号链接，避免循环。
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            total += dir_size(&entry.path());
        } else if metadata.is_file() {
            total += metadata.len();
        }
    }
    total
}

/// 按实际目录大小修复已有 ACF 的 `SizeOnDisk` / `StateFlags` / `LastUpdated`。
pub fn repair(steamapps: &Path, app_id: u32, size_on_disk: u64) -> Result<PathBuf, String> {
    let target = path(steamapps, app_id);
    let text = fs::read_to_string(&target)
        .map_err(|error| format!("无法读取 {}：{error}", target.display()))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for line in &mut lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("\"SizeOnDisk\"") {
            *line = format!("\t\"SizeOnDisk\"\t\t\"{size_on_disk}\"");
        } else if trimmed.starts_with("\"StateFlags\"") {
            *line = "\t\"StateFlags\"\t\t\"4\"".to_owned();
        } else if trimmed.starts_with("\"LastUpdated\"") {
            *line = format!("\t\"LastUpdated\"\t\t\"{now}\"");
        }
    }
    let backup = target.with_extension("acf.bak");
    fs::copy(&target, &backup)
        .map_err(|error| format!("无法备份 {}：{error}", backup.display()))?;
    let mut output = lines.join("\n");
    output.push('\n');
    fs::write(&target, output)
        .map_err(|error| format!("无法写入 {}：{error}", target.display()))?;
    Ok(target)
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
    fn repairs_size_and_state_flags() {
        let directory = tempfile::tempdir().unwrap();
        let steamapps = directory.path();
        let target = path(steamapps, 10);
        fs::write(
            &target,
            "\"AppState\"\n{\n\t\"StateFlags\"\t\t\"0\"\n\t\"SizeOnDisk\"\t\t\"0\"\n\t\"InstalledDepots\"\n\t{\n\t}\n}\n",
        )
        .unwrap();
        repair(steamapps, 10, 4096).unwrap();
        let text = fs::read_to_string(&target).unwrap();
        assert!(text.contains("\"SizeOnDisk\"\t\t\"4096\""));
        assert!(text.contains("\"StateFlags\"\t\t\"4\""));
        assert!(target.with_extension("acf.bak").exists());
    }

    #[test]
    fn sums_directory_size() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("a.bin"), vec![0u8; 100]).unwrap();
        fs::create_dir(directory.path().join("sub")).unwrap();
        fs::write(directory.path().join("sub/b.bin"), vec![0u8; 50]).unwrap();
        assert_eq!(dir_size(directory.path()), 150);
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
