//! 把外部下载好的 `<depot>_<manifest>.manifest` 放进 Steam 的 depotcache。
//!
//! Steam 会从 `depotcache` 读取固定版本的 manifest；配合 SLSsteam 的 `ManifestIds`
//! 就能锁定版本。文件本身由用户提供（GameHaven 不内置第三方 manifest 下载）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 公开的 manifest 镜像仓库：根目录平铺 `<depot>_<manifest>.manifest`，无需 key。
const MIRROR_REPOS: &[&str] = &[
    "qwe213312/k25FCdfEOoEJ42S6",
    "mejikuhibiniu1/k25FCdfEOoEJ42S6",
    "Sainan/k25FCdfEOoEJ42S6",
];

#[derive(Clone, Debug)]
pub struct ManifestFile {
    pub depot_id: u32,
    pub manifest_id: u64,
    pub path: PathBuf,
}

/// 从 `<depot>_<manifest>.manifest` 解析 depot 与 manifest ID。
pub fn parse(path: &Path) -> Option<ManifestFile> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".manifest")?;
    let (depot, manifest) = stem.split_once('_')?;
    Some(ManifestFile {
        depot_id: depot.parse().ok()?,
        manifest_id: manifest.parse().ok()?,
        path: path.to_owned(),
    })
}

/// 各镜像的 raw URL，按顺序尝试。
pub fn source_urls(depot_id: u32, manifest_id: u64) -> Vec<String> {
    MIRROR_REPOS
        .iter()
        .map(|repo| {
            format!(
                "https://raw.githubusercontent.com/{repo}/main/{depot_id}_{manifest_id}.manifest"
            )
        })
        .collect()
}

/// 从公开镜像下载 manifest 字节。仓库未收录时返回错误。
pub fn download(depot_id: u32, manifest_id: u64) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    let mut last_error = None;
    for url in source_urls(depot_id, manifest_id) {
        match agent.get(&url).call() {
            Ok(mut response) if response.status().is_success() => {
                match response
                    .body_mut()
                    .with_config()
                    .limit(16 * 1024 * 1024)
                    .read_to_vec()
                {
                    Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
                    Ok(_) => last_error = Some("镜像返回空文件。".to_owned()),
                    Err(error) => last_error = Some(format!("读取失败：{error}")),
                }
            }
            Ok(response) => {
                last_error = Some(format!("HTTP {}", response.status().as_u16()));
            }
            Err(ureq::Error::StatusCode(404)) => continue,
            Err(error) => last_error = Some(format!("请求失败：{error}")),
        }
    }
    Err(format!(
        "公开镜像里没有 {depot_id}_{manifest_id}.manifest{}",
        last_error
            .map(|error| format!("（{error}）"))
            .unwrap_or_default()
    ))
}

/// 把下载到的字节保存到缓存目录，返回可供 [`install`] 使用的文件。
pub fn cache_bytes(depot_id: u32, manifest_id: u64, bytes: &[u8]) -> Result<ManifestFile, String> {
    let home = std::env::home_dir().ok_or_else(|| "无法确定缓存目录。".to_owned())?;
    let directory = home.join(".cache/gamehaven/manifests");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("无法创建 {}：{error}", directory.display()))?;
    let path = directory.join(format!("{depot_id}_{manifest_id}.manifest"));
    fs::write(&path, bytes).map_err(|error| format!("无法写入 {}：{error}", path.display()))?;
    Ok(ManifestFile {
        depot_id,
        manifest_id,
        path,
    })
}

/// 复制到 Steam 根、`config/depotcache` 和当前库的 `steamapps/depotcache`。
pub fn install(
    steam_root: &Path,
    library: &Path,
    file: &ManifestFile,
) -> Result<Vec<PathBuf>, String> {
    let name = file
        .path
        .file_name()
        .ok_or_else(|| "manifest 文件没有文件名。".to_owned())?;
    let targets = [
        steam_root.join("depotcache").join(name),
        steam_root.join("config/depotcache").join(name),
        library.join("steamapps/depotcache").join(name),
    ];
    let mut written = Vec::new();
    for target in targets {
        if target == file.path {
            written.push(target);
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
        }
        fs::copy(&file.path, &target)
            .map_err(|error| format!("无法写入 {}：{error}", target.display()))?;
        written.push(target);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_depot_and_manifest_from_filename() {
        let file = parse(Path::new("/tmp/4115451_7450216563795868763.manifest")).unwrap();
        assert_eq!(file.depot_id, 4115451);
        assert_eq!(file.manifest_id, 7450216563795868763);
        assert!(parse(Path::new("/tmp/renamed.manifest")).is_none());
        assert!(parse(Path::new("/tmp/4115451_7450216563795868763.txt")).is_none());
    }

    #[test]
    fn builds_mirror_urls_in_order() {
        let urls = source_urls(2483191, 3954142684088109524);
        assert_eq!(urls.len(), 3);
        assert!(urls[0].starts_with("https://raw.githubusercontent.com/qwe213312/"));
        assert!(urls[0].ends_with("/2483191_3954142684088109524.manifest"));
    }

    #[test]
    fn installs_to_all_depotcache_locations() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let steam = root.join("Steam");
        let library = root.join("Library");
        let source = root.join("4115451_7450216563795868763.manifest");
        fs::write(&source, b"manifest-bytes").unwrap();
        let file = parse(&source).unwrap();
        let written = install(&steam, &library, &file).unwrap();
        assert_eq!(written.len(), 3);
        for path in written {
            assert_eq!(fs::read(&path).unwrap(), b"manifest-bytes");
        }
        assert!(steam.join("depotcache/4115451_7450216563795868763.manifest").exists());
        assert!(library.join("steamapps/depotcache/4115451_7450216563795868763.manifest").exists());
    }
}
