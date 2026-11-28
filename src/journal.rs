//! 本地操作日志与写前备份：记录对 Steam / SLSsteam 文件做过的写操作，支持撤销最近一次。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Entry {
    pub at: u64,
    pub message: String,
}

fn log_path() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    Some(home.join(".local/share/gamehaven/journal.log"))
}

fn undo_root() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    Some(home.join(".local/share/gamehaven/undo"))
}

pub fn record(message: &str) {
    let Some(path) = log_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let now = now_secs();
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{now}\t{message}");
    }
}

pub fn tail(limit: usize) -> Vec<Entry> {
    let Some(path) = log_path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .rev()
        .take(limit)
        .filter_map(|line| {
            let (at, message) = line.split_once('\t')?;
            Some(Entry {
                at: at.parse().ok()?,
                message: message.to_owned(),
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// 把时间戳转成「刚刚 / N 分钟前 / N 小时前 / N 天前」。
pub fn relative(at: u64) -> String {
    let now = now_secs();
    let seconds = now.saturating_sub(at);
    if seconds < 60 {
        "刚刚".to_owned()
    } else if seconds < 3600 {
        format!("{} 分钟前", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} 小时前", seconds / 3600)
    } else {
        format!("{} 天前", seconds / 86_400)
    }
}

/// 在写操作前调用：把要改的文件复制到撤销目录。
pub fn backup(label: &str, paths: &[&Path]) -> Option<PathBuf> {
    backup_at(&undo_root()?, label, paths)
}

/// 撤销最近一次写操作，返回被恢复的文件数量。
pub fn undo_last() -> Result<String, String> {
    let root = undo_root().ok_or_else(|| "无法确定撤销目录。".to_owned())?;
    undo_last_at(&root)
}

/// 最近一次可撤销操作的描述，例如「添加游戏 · 2 分钟前」。
pub fn undo_info() -> Option<String> {
    let root = undo_root()?;
    let dir = latest_undo_dir(&root)?;
    let name = dir.file_name()?.to_string_lossy();
    let (at, label) = name.split_once('_')?;
    let at: u64 = at.parse().ok()?;
    Some(format!("{label} · {}", relative(at)))
}

pub(crate) fn backup_at(root: &Path, label: &str, paths: &[&Path]) -> Option<PathBuf> {
    let dir = root.join(format!("{}_{}", now_millis(), sanitize(label)));
    fs::create_dir_all(&dir).ok()?;
    let mut manifest = String::new();
    let mut copied = 0usize;
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = dir.join(name);
        if fs::copy(path, &target).is_ok() {
            manifest.push_str(&format!("{}\t{}\n", path.display(), target.display()));
            copied += 1;
        }
    }
    if copied == 0 {
        let _ = fs::remove_dir_all(&dir);
        return None;
    }
    fs::write(dir.join("manifest.txt"), manifest).ok()?;
    Some(dir)
}

pub(crate) fn undo_last_at(root: &Path) -> Result<String, String> {
    let dir = latest_undo_dir(root).ok_or_else(|| "没有可撤销的操作。".to_owned())?;
    let manifest = fs::read_to_string(dir.join("manifest.txt"))
        .map_err(|error| format!("无法读取撤销记录：{error}"))?;
    let mut restored = 0usize;
    for line in manifest.lines() {
        let Some((original, backup)) = line.split_once('\t') else {
            continue;
        };
        if let Some(parent) = Path::new(original).parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::copy(backup, original)
            .map_err(|error| format!("无法恢复 {original}：{error}"))?;
        restored += 1;
    }
    fs::remove_dir_all(&dir).map_err(|error| format!("无法清理撤销目录：{error}"))?;
    record(&format!("撤销最近一次写操作（恢复 {restored} 个文件）"));
    Ok(format!("已撤销最近一次写操作，恢复 {restored} 个文件。"))
}

fn latest_undo_dir(root: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.pop()
}

fn sanitize(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backs_up_and_restores_files() {
        let root = tempfile::tempdir().unwrap();
        let undo = root.path().join("undo");
        let file = root.path().join("config.yaml");
        fs::write(&file, "before").unwrap();
        backup_at(&undo, "编辑配置", &[&file]).unwrap();
        fs::write(&file, "after").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "after");
        undo_last_at(&undo).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "before");
        assert!(latest_undo_dir(&undo).is_none());
    }

    #[test]
    fn skips_missing_files() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");
        assert!(backup_at(&root.path().join("undo"), "x", &[&missing]).is_none());
    }
}
