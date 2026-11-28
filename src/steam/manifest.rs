//! 把外部下载好的 `<depot>_<manifest>.manifest` 放进 Steam 的 depotcache。
//!
//! Steam 会从 `depotcache` 读取固定版本的 manifest；配合 SLSsteam 的 `ManifestIds`
//! 就能锁定版本。文件本身由用户提供（GameHaven 不内置第三方 manifest 下载）。

use std::fs;
use std::path::{Path, PathBuf};

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
