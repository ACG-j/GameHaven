//! 本地操作日志：记录对 Steam / SLSsteam 文件做过的写操作，方便回查。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Entry {
    pub at: u64,
    pub message: String,
}

fn log_path() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    Some(home.join(".local/share/gamehaven/journal.log"))
}

pub fn record(message: &str) {
    let Some(path) = log_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{now}\t{message}");
    }
}

pub fn tail(limit: usize) -> Vec<Entry> {
    let Some(path) = log_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(at);
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
