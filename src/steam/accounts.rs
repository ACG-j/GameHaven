//! 读取 Steam 的本地账号列表，并切换自动登录账号。
//!
//! 只改 `config/config.vdf` 的 `AutoLoginUser`，不碰密码和令牌；Steam 必须退出后修改，
//! 下次启动才会用新账号自动登录。

use std::fs;
use std::path::Path;

use keyvalues_parser::{Obj, Value};

#[derive(Clone, Debug)]
pub struct Account {
    pub steam_id: String,
    pub account_name: String,
    pub persona_name: String,
    pub most_recent: bool,
    pub timestamp: u64,
}

/// 读取 `config/loginusers.vdf`，按「最近使用」优先排序。
pub fn list(steam_root: &Path) -> Vec<Account> {
    let path = steam_root.join("config/loginusers.vdf");
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(vdf) = keyvalues_parser::parse(text.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    let value: Value<'static> = vdf.into_vdf().into_owned().value;
    let Some(users) = value.get_obj() else {
        return Vec::new();
    };
    let mut accounts: Vec<Account> = users
        .iter()
        .filter_map(|(steam_id, values)| {
            let object = values.first()?.get_obj()?;
            let account_name = field(object, "AccountName")?.to_owned();
            if account_name.is_empty() {
                return None;
            }
            Some(Account {
                steam_id: steam_id.to_string(),
                account_name,
                persona_name: field(object, "PersonaName").unwrap_or("").to_owned(),
                most_recent: field(object, "MostRecent").is_some_and(|value| value == "1"),
                timestamp: field(object, "Timestamp")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
            })
        })
        .collect();
    accounts.sort_by(|a, b| {
        b.most_recent
            .cmp(&a.most_recent)
            .then(b.timestamp.cmp(&a.timestamp))
    });
    accounts
}

/// 当前自动登录账号：优先 `MostRecent`，其次 `config.vdf` 的 `AutoLoginUser`。
pub fn current(steam_root: &Path) -> Option<String> {
    let accounts = list(steam_root);
    if let Some(account) = accounts.iter().find(|account| account.most_recent) {
        return Some(account.account_name.clone());
    }
    if let Ok(text) = fs::read_to_string(steam_root.join("config/config.vdf")) {
        if let Some(name) = text
            .lines()
            .find(|line| line.contains("\"AutoLoginUser\""))
            .and_then(|line| line.split('"').nth(3))
        {
            return Some(name.to_owned());
        }
    }
    // 没有 MostRecent / AutoLoginUser 时，用时间戳最新的账号兜底。
    accounts.first().map(|account| account.account_name.clone())
}

/// 把 `account_name` 设为自动登录账号：改写 `loginusers.vdf` 的 `MostRecent`，
/// 如果 `config.vdf` 里存在 `AutoLoginUser` 也一并更新。Steam 运行中修改会被覆盖。
pub fn switch(steam_root: &Path, account_name: &str) -> Result<(), String> {
    let target = list(steam_root)
        .into_iter()
        .find(|account| account.account_name == account_name)
        .ok_or_else(|| format!("没有找到账号 {account_name}。"))?;

    let path = steam_root.join("config/loginusers.vdf");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let blocks = account_blocks(&lines);
    if blocks.is_empty() {
        return Err("loginusers.vdf 里没有可切换的账号。".to_owned());
    }
    // 从后往前改，避免插入行影响前面的块索引。
    for block in blocks.iter().rev() {
        let value = if block.id == target.steam_id { "1" } else { "0" };
        let existing = (block.open + 1..block.close)
            .find(|index| lines[*index].trim().starts_with("\"MostRecent\""));
        let line = format!("\t\t\"MostRecent\"\t\t\"{value}\"");
        match existing {
            Some(index) => lines[index] = line,
            None => lines.insert(block.open + 1, line),
        }
    }
    let backup = path.with_extension("vdf.bak");
    fs::copy(&path, &backup).map_err(|error| format!("无法备份 loginusers.vdf：{error}"))?;
    fs::write(&path, format!("{}\n", lines.join("\n")))
        .map_err(|error| format!("无法写入 loginusers.vdf：{error}"))?;

    // 旧版 Steam 还会读 AutoLoginUser；存在时同步，不存在不当作失败。
    if let Some(config) = write_auto_login_user(steam_root, account_name) {
        return Err(config);
    }
    Ok(())
}

struct Block {
    id: String,
    open: usize,
    close: usize,
}

/// 找出 `loginusers.vdf` 里每个 `"steamid" { ... }` 块的行号。
fn account_blocks(lines: &[String]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut open: Option<(String, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some((id, brace)) = &open {
            if trimmed == "}" {
                blocks.push(Block {
                    id: id.clone(),
                    open: *brace,
                    close: index,
                });
                open = None;
            }
            continue;
        }
        if trimmed.len() > 5
            && trimmed.starts_with('"')
            && trimmed.ends_with('"')
            && trimmed
                .trim_matches('"')
                .bytes()
                .all(|byte| byte.is_ascii_digit())
            && lines.get(index + 1).is_some_and(|next| next.trim() == "{")
        {
            open = Some((trimmed.trim_matches('"').to_owned(), index + 1));
        }
    }
    blocks
}

fn write_auto_login_user(steam_root: &Path, account_name: &str) -> Option<String> {
    let path = steam_root.join("config/config.vdf");
    let text = fs::read_to_string(&path).ok()?;
    if !text.contains("\"AutoLoginUser\"") {
        return None;
    }
    let mut output = String::with_capacity(text.len() + 64);
    for line in text.lines() {
        if line.contains("\"AutoLoginUser\"") {
            let indent: String = line
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect();
            output.push_str(&indent);
            output.push_str("\"AutoLoginUser\"\t\t\"");
            output.push_str(account_name);
            output.push_str("\"\n");
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    let backup = path.with_extension("vdf.bak");
    if let Err(error) = fs::copy(&path, &backup) {
        return Some(format!("无法备份 config.vdf：{error}"));
    }
    fs::write(&path, output)
        .map_err(|error| format!("无法写入 config.vdf：{error}"))
        .err()
}

fn field<'a>(object: &'a Obj<'_>, name: &str) -> Option<&'a str> {
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))?
        .1
        .first()?
        .get_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(root: &Path) {
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(
            root.join("config/loginusers.vdf"),
            r#""users"
{
    "76561199107044841"
    {
        "AccountName" "alice"
        "PersonaName" "Alice"
        "MostRecent" "1"
        "Timestamp" "1700000000"
    }
    "76561198000000000"
    {
        "AccountName" "bob"
        "PersonaName" "Bob"
        "MostRecent" "0"
        "Timestamp" "1600000000"
    }
}"#,
        )
        .unwrap();
        fs::write(
            root.join("config/config.vdf"),
            r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "AutoLoginUser" "alice"
            }
        }
    }
}"#,
        )
        .unwrap();
    }

    #[test]
    fn lists_accounts_with_recent_first() {
        let directory = tempfile::tempdir().unwrap();
        setup(directory.path());
        let accounts = list(directory.path());
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].account_name, "alice");
        assert_eq!(accounts[1].persona_name, "Bob");
        assert_eq!(current(directory.path()).as_deref(), Some("alice"));
    }

    #[test]
    fn switches_auto_login_and_keeps_backup() {
        let directory = tempfile::tempdir().unwrap();
        setup(directory.path());
        switch(directory.path(), "bob").unwrap();
        assert_eq!(current(directory.path()).as_deref(), Some("bob"));
        assert!(directory.path().join("config/loginusers.vdf.bak").exists());
        let accounts = list(directory.path());
        assert!(accounts.iter().any(|account| account.account_name == "bob" && account.most_recent));
        assert!(accounts.iter().any(|account| account.account_name == "alice" && !account.most_recent));
    }
}
