use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use rustix::fs::{FlockOperation, OFlags, flock, open};

const HEADER: &[u8] = b"\0shortcuts\0";
const END: u8 = 8;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub enum Import {
    Added,
    AlreadyExists,
}

pub fn add(
    config: &Path,
    name: &str,
    executable: &Path,
    directory: &Path,
) -> Result<Import, String> {
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_control) {
        return Err("请输入有效的游戏名称。".into());
    }
    let executable = executable
        .canonicalize()
        .map_err(|err| format!("无法打开启动文件（{err}）。"))?;
    let directory = directory
        .canonicalize()
        .map_err(|err| format!("无法打开工作目录（{err}）。"))?;
    let metadata = executable.metadata().map_err(|err| err.to_string())?;
    let windows = executable
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !metadata.is_file() || (!windows && metadata.permissions().mode() & 0o111 == 0) {
        return Err("请选择有执行权限的 Linux 程序或 Windows .exe。".into());
    }
    if !directory.is_dir() {
        return Err("请选择一个工作目录。".into());
    }
    let executable = quote_path(&executable)?;
    let directory = quote_path(&directory)?;

    // Steam saves its in-memory shortcuts on exit.
    if steam_running() {
        return Err("请退出 Steam 客户端后再添加游戏。".into());
    }
    let lock = open(
        config.join(".gamehaven-shortcuts.lock"),
        OFlags::WRONLY | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .map_err(|err| format!("无法锁定 Steam 配置（{err}）。"))?;
    flock(&lock, FlockOperation::NonBlockingLockExclusive)
        .map_err(|err| format!("无法锁定 Steam 配置（{err}）。"))?;

    let destination = config.join("shortcuts.vdf");
    let existing =
        read_existing(&destination).map_err(|err| format!("无法读取快捷方式（{err}）。"))?;
    let initial = b"\0shortcuts\0\x08\x08";
    let original = existing.as_deref().unwrap_or(initial);
    let Some(updated) = append_entry(original, name, &executable, &directory)? else {
        return Ok(Import::AlreadyExists);
    };
    let mut temp = tempfile::NamedTempFile::new_in(config).map_err(|err| err.to_string())?;
    temp.write_all(&updated)
        .and_then(|()| temp.as_file().sync_all())
        .map_err(|err| format!("无法写入快捷方式（{err}）。"))?;
    if steam_running() {
        return Err("Steam 客户端已启动，请退出后重试。".into());
    }
    if read_existing(&destination).map_err(|err| err.to_string())? != existing {
        return Err("Steam 快捷方式已被其他程序修改，请重试。".into());
    }
    temp.persist(&destination)
        .map_err(|err| format!("无法保存快捷方式（{err}）。"))?;
    File::open(config)
        .and_then(|directory| directory.sync_all())
        .map_err(|err| format!("无法同步 Steam 配置目录（{err}）。"))?;
    Ok(Import::Added)
}

fn read_existing(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    ) {
        Ok(fd) => {
            let file = File::from(fd);
            if !file.metadata()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "shortcuts.vdf is not a regular file",
                ));
            }
            let mut content = Vec::new();
            file.take(MAX_FILE_BYTES + 1).read_to_end(&mut content)?;
            if content.len() as u64 > MAX_FILE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "shortcuts.vdf exceeds 16 MiB",
                ));
            }
            Ok(Some(content))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn steam_running() -> bool {
    let Ok(processes) = fs::read_dir("/proc") else {
        return false;
    };
    let uid = rustix::process::getuid().as_raw();
    processes.flatten().any(|process| {
        process
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            && process
                .metadata()
                .is_ok_and(|metadata| metadata.uid() == uid)
            && fs::read_to_string(process.path().join("stat")).is_ok_and(|stat| {
                stat.split_once('(')
                    .and_then(|(_, rest)| rest.rsplit_once(") "))
                    .is_some_and(|(name, status)| name == "steam" && !status.starts_with('Z'))
            })
    })
}

fn quote_path(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .filter(|text| !text.contains(['"', '\\']) && !text.contains(char::is_control))
        .ok_or("Steam 路径需要使用 UTF-8，且不能含引号、反斜杠或控制字符。")?;
    Ok(format!("\"{text}\""))
}

fn append_entry(
    original: &[u8],
    name: &str,
    executable: &str,
    directory: &str,
) -> Result<Option<Vec<u8>>, String> {
    edit_shortcuts(original, name, executable, directory)
        .ok_or_else(|| "无法读取现有 shortcuts.vdf，请检查 Steam 配置。".into())
}

fn edit_shortcuts(
    original: &[u8],
    name: &str,
    executable: &str,
    directory: &str,
) -> Option<Option<Vec<u8>>> {
    let mut rest = original.strip_prefix(HEADER)?;
    let mut slot = 0u32;
    let mut hash = crc32fast::Hasher::new();
    hash.update(executable.as_bytes());
    hash.update(name.as_bytes());
    let app_id = hash.finalize() | 0x8000_0000;
    let mut duplicate = false;
    let mut id_collision = false;
    while let Some(body) = rest.strip_prefix(&[0]) {
        rest = body;
        let index = std::str::from_utf8(cstring(&mut rest)?)
            .ok()?
            .parse::<u32>()
            .ok()?;
        slot = slot.max(index.checked_add(1)?);
        let mut old_name = None;
        let mut old_exe = None;
        let mut old_app_id = None;
        read_fields(&mut rest, 0, &mut |key, payload| match key {
            key if key.eq_ignore_ascii_case(b"AppName") => old_name = payload.text(),
            key if key.eq_ignore_ascii_case(b"Exe") => old_exe = payload.text(),
            key if key.eq_ignore_ascii_case(b"appid") => old_app_id = payload.int(),
            _ => {}
        })?;
        duplicate |= old_name == Some(name.as_bytes())
            && old_exe.is_some_and(|prev| unquote(prev) == unquote(executable.as_bytes()));
        id_collision |= old_app_id == Some(app_id);
    }
    if !matches!(rest, [END] | [END, END]) {
        return None;
    }
    if duplicate {
        return Some(None);
    }
    if id_collision {
        return None;
    }

    let mut bytes = original[..original.len() - rest.len()].to_vec();
    put_key(&mut bytes, 0, &slot.to_string());
    put_key(&mut bytes, 2, "appid");
    bytes.extend(app_id.to_le_bytes());
    for (key, value) in [
        ("AppName", name),
        ("Exe", executable),
        ("StartDir", directory),
        ("icon", ""),
        ("ShortcutPath", ""),
        ("LaunchOptions", ""),
    ] {
        put_key(&mut bytes, 1, key);
        bytes.extend(value.as_bytes());
        bytes.push(0);
    }
    for (key, value) in [
        ("IsHidden", 0u32),
        ("AllowDesktopConfig", 1),
        ("AllowOverlay", 1),
        ("LastPlayTime", 0),
    ] {
        put_key(&mut bytes, 2, key);
        bytes.extend(value.to_le_bytes());
    }
    put_key(&mut bytes, 0, "tags");
    bytes.extend([END, END]);
    bytes.extend(rest);
    Some(Some(bytes))
}

fn unquote(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_prefix(b"\"")
        .and_then(|suffix| suffix.strip_suffix(b"\""))
        .unwrap_or(bytes)
}

fn put_key(bytes: &mut Vec<u8>, kind: u8, name: &str) {
    bytes.push(kind);
    bytes.extend(name.as_bytes());
    bytes.push(0);
}

fn cstring<'a>(rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    let end = rest.iter().position(|&byte| byte == 0)?;
    let (text, end) = rest.split_at(end);
    *rest = &end[1..];
    Some(text)
}

enum Field<'a> {
    Text(&'a [u8]),
    Int(u32),
}

impl<'a> Field<'a> {
    fn text(self) -> Option<&'a [u8]> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    fn int(self) -> Option<u32> {
        match self {
            Self::Int(value) => Some(value),
            _ => None,
        }
    }
}

fn read_fields<'a>(
    rest: &mut &'a [u8],
    depth: usize,
    visit: &mut impl FnMut(&'a [u8], Field<'a>),
) -> Option<()> {
    if depth > 16 {
        return None;
    }
    loop {
        let (&kind, tail) = rest.split_first()?;
        *rest = tail;
        if kind == END {
            return Some(());
        }
        let name = cstring(rest)?;
        let value = match kind {
            0 => {
                read_fields(rest, depth + 1, visit)?;
                continue;
            }
            1 => Field::Text(cstring(rest)?),
            2..=4 | 6 => {
                let (value, tail) = rest.split_at_checked(4)?;
                *rest = tail;
                if kind != 2 {
                    continue;
                }
                Field::Int(u32::from_le_bytes(value.try_into().ok()?))
            }
            7 => {
                *rest = rest.get(8..)?;
                continue;
            }
            _ => return None,
        };
        if depth == 0 {
            visit(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_without_rewriting_existing_fields_and_avoids_duplicates() {
        let original = b"\0shortcuts\0\x00\x31\x00\x01UnrecognisedField\0original\0\x08\x08\x08";
        let updated = append_entry(original, "Test", "\"/tmp/Game Test.exe\"", "\"/tmp\"")
            .unwrap()
            .unwrap();
        assert!(updated.starts_with(&original[..original.len() - 2]));
        assert!(updated.windows(13).any(|bytes| bytes == b"Game Test.exe"));
        assert!(
            append_entry(&updated, "Test", "\"/tmp/Game Test.exe\"", "\"/tmp\"")
                .unwrap()
                .is_none()
        );
        assert!(append_entry(&updated[..updated.len() - 3], "other", "exe", "dir").is_err());
    }
}
