//! 对 SLSsteam 的 `config.yaml` 做最小化的行级编辑。
//!
//! 只改目标行，保留注释、空行和原有顺序；每次写入前备份为 `config.yaml.bak`，
//! 写入走临时文件 + rename，避免半截文件。

use std::fs;
use std::path::{Path, PathBuf};

pub struct Config {
    path: PathBuf,
}

#[allow(dead_code)] // remove_app / add_dlc / add_token / set_manifest 供后续页面使用。
impl Config {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 把 AppID 追加到 `AdditionalApps`。已经存在时返回 `Ok(false)`。
    pub fn add_app(&self, app_id: u32) -> Result<bool, String> {
        self.edit(|lines| {
            let section = find_section(lines, "AdditionalApps")
                .ok_or_else(|| "config.yaml 里没有 AdditionalApps 段。".to_owned())?;
            if list_contains(lines, &section, app_id) {
                return Ok(false);
            }
            insert_list_item(lines, &section, format!("  - {app_id}"));
            Ok(true)
        })
    }

    /// 从 `AdditionalApps` 移除 AppID。不存在时返回 `Ok(false)`。
    pub fn remove_app(&self, app_id: u32) -> Result<bool, String> {
        self.edit(|lines| {
            let Some(section) = find_section(lines, "AdditionalApps") else {
                return Ok(false);
            };
            let needle = app_id.to_string();
            let mut removed = false;
            let mut index = section.header + 1;
            while index < lines.len() {
                let trimmed = lines[index].trim();
                let matches = trimmed
                    .strip_prefix('-')
                    .and_then(|rest| rest.trim().split_whitespace().next())
                    .is_some_and(|value| value == needle);
                if matches {
                    lines.remove(index);
                    removed = true;
                    break;
                }
                if index >= section.end {
                    break;
                }
                index += 1;
            }
            Ok(removed)
        })
    }

    pub fn has_app(&self, app_id: u32) -> bool {
        match self.read() {
            Ok((lines, _)) => find_section(&lines, "AdditionalApps")
                .is_some_and(|section| list_contains(&lines, &section, app_id)),
            Err(_) => false,
        }
    }

    /// 在 `DlcData` 里登记 DLC。已经存在时返回 `Ok(false)`。
    pub fn add_dlc(&self, parent: u32, dlc: u32, name: &str) -> Result<bool, String> {
        self.edit(|lines| {
            let section = find_section(lines, "DlcData")
                .ok_or_else(|| "config.yaml 里没有 DlcData 段。".to_owned())?;
            let parent_line = format!("  {parent}:");
            let parent_index = (section.header + 1..section.end)
                .find(|index| lines[*index].trim_end() == parent_line);
            let dlc_name = yaml_string(name);

            if let Some(parent_index) = parent_index {
                let child_end = child_block_end(lines, parent_index, section.end);
                let exists = (parent_index + 1..child_end).any(|index| {
                    lines[index]
                        .trim_start()
                        .strip_prefix(&format!("{dlc}:"))
                        .is_some()
                });
                if exists {
                    return Ok(false);
                }
                lines.insert(child_end, format!("    {dlc}: {dlc_name}"));
            } else {
                insert_list_item(
                    lines,
                    &section,
                    format!("  {parent}:\n    {dlc}: {dlc_name}"),
                );
            }
            Ok(true)
        })
    }

    /// 在 `AppTokens` 里写入 AppID 的访问令牌。已经存在时返回 `Ok(false)`，不覆盖旧值。
    pub fn add_token(&self, app_id: u32, token: &str) -> Result<bool, String> {
        self.edit(|lines| {
            let section = find_section(lines, "AppTokens")
                .ok_or_else(|| "config.yaml 里没有 AppTokens 段。".to_owned())?;
            let prefix = format!("{app_id}:");
            let exists = (section.header + 1..section.end).any(|index| {
                lines[index].trim_start().starts_with(&prefix)
            });
            if exists {
                return Ok(false);
            }
            insert_list_item(
                lines,
                &section,
                format!("  {app_id}: {}", yaml_string(token)),
            );
            Ok(true)
        })
    }

    /// 在 `ManifestIds` 里锁定 depot 的 manifest。已存在时替换旧值。
    pub fn set_manifest(&self, depot: u32, manifest: u64) -> Result<bool, String> {
        self.edit(|lines| {
            let section = find_section(lines, "ManifestIds")
                .ok_or_else(|| "config.yaml 里没有 ManifestIds 段。".to_owned())?;
            let prefix = format!("{depot}:");
            let replacement = format!("  {depot}: {manifest}");
            if let Some(index) = (section.header + 1..section.end)
                .find(|index| lines[*index].trim_start().starts_with(&prefix))
            {
                if lines[index].trim() == replacement.trim() {
                    return Ok(false);
                }
                lines[index] = replacement;
                return Ok(true);
            }
            insert_list_item(lines, &section, replacement);
            Ok(true)
        })
    }

    fn read(&self) -> Result<(Vec<String>, bool), String> {
        let text = fs::read_to_string(&self.path)
            .map_err(|error| format!("无法读取 {}：{error}", self.path.display()))?;
        let trailing = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(&text);
        Ok((body.split('\n').map(str::to_owned).collect(), trailing))
    }

    fn edit(
        &self,
        change: impl FnOnce(&mut Vec<String>) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let (mut lines, trailing) = self.read()?;
        let changed = change(&mut lines)?;
        if !changed {
            return Ok(false);
        }
        self.write(&lines, trailing)?;
        Ok(true)
    }

    fn write(&self, lines: &[String], trailing: bool) -> Result<(), String> {
        let mut content = lines.join("\n");
        if trailing {
            content.push('\n');
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
        }
        if self.path.exists() {
            let backup = self.path.with_extension("yaml.bak");
            fs::copy(&self.path, &backup)
                .map_err(|error| format!("无法备份到 {}：{error}", backup.display()))?;
        }
        let temporary = self.path.with_extension("yaml.tmp");
        fs::write(&temporary, content.as_bytes())
            .map_err(|error| format!("无法写入 {}：{error}", temporary.display()))?;
        if let Ok(metadata) = fs::metadata(&self.path) {
            let _ = fs::set_permissions(&temporary, metadata.permissions());
        }
        fs::rename(&temporary, &self.path)
            .map_err(|error| format!("无法替换 {}：{error}", self.path.display()))?;
        Ok(())
    }
}

struct Section {
    header: usize,
    end: usize,
}

fn find_section(lines: &[String], key: &str) -> Option<Section> {
    let prefix = format!("{key}:");
    let header = lines.iter().position(|line| {
        !line.starts_with(' ')
            && !line.starts_with('\t')
            && line.trim_start().starts_with(&prefix)
    })?;
    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(header + 1) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            end = index;
            break;
        }
    }
    Some(Section { header, end })
}

fn insert_list_item(lines: &mut Vec<String>, section: &Section, item: String) {
    let mut insert_at = section.header + 1;
    for index in (section.header + 1..section.end).rev() {
        let trimmed = lines[index].trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            insert_at = index + 1;
            break;
        }
    }
    for (offset, line) in item.split('\n').enumerate() {
        lines.insert(insert_at + offset, line.to_owned());
    }
}

fn list_contains(lines: &[String], section: &Section, value: u32) -> bool {
    let needle = value.to_string();
    (section.header + 1..section.end).any(|index| {
        lines[index]
            .trim()
            .strip_prefix('-')
            .and_then(|rest| rest.trim().split_whitespace().next())
            .is_some_and(|item| item == needle)
    })
}

fn child_block_end(lines: &[String], parent: usize, section_end: usize) -> usize {
    let mut end = section_end;
    for index in parent + 1..section_end {
        let line = &lines[index];
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.chars().take_while(|character| *character == ' ').count();
        if indent <= 2 {
            end = index;
            break;
        }
    }
    end
}

fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::Config;

    fn config_with(text: &str) -> (TempDir, Config) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        fs::write(&path, text).unwrap();
        (directory, Config::new(path))
    }

    const SAMPLE: &str = "\
# comment
AppIds:

AdditionalApps:
  - 1493710   # migrated
  - 228980

DlcData:
  2483190:
    4439240: \"Old DLC\"

AppTokens:
  2483190: \"2264898939686399139\"

ManifestIds:
  2483191: 3954142684088109524
";

    #[test]
    fn adds_app_and_keeps_comments() {
        let (directory, config) = config_with(SAMPLE);
        assert!(config.add_app(4115450).unwrap());
        let text = fs::read_to_string(directory.path().join("config.yaml")).unwrap();
        assert!(text.contains("# comment"));
        assert!(text.contains("  - 1493710   # migrated"));
        assert!(text.contains("  - 4115450"));
        assert!(!config.add_app(4115450).unwrap());
    }

    #[test]
    fn removes_app() {
        let (directory, config) = config_with(SAMPLE);
        assert!(config.remove_app(228980).unwrap());
        let text = fs::read_to_string(directory.path().join("config.yaml")).unwrap();
        assert!(!text.contains("  - 228980"));
        assert!(text.contains("  - 1493710"));
        assert!(!config.remove_app(228980).unwrap());
    }

    #[test]
    fn adds_dlc_below_existing_parent() {
        let (directory, config) = config_with(SAMPLE);
        assert!(config.add_dlc(2483190, 4439250, "Expansion 1").unwrap());
        let text = fs::read_to_string(directory.path().join("config.yaml")).unwrap();
        assert!(text.contains("    4439250: \"Expansion 1\""));
        assert!(!config.add_dlc(2483190, 4439250, "Expansion 1").unwrap());
    }

    #[test]
    fn adds_new_dlc_parent_block() {
        let (directory, config) = config_with(SAMPLE);
        assert!(config.add_dlc(1000, 1001, "DLC").unwrap());
        let text = fs::read_to_string(directory.path().join("config.yaml")).unwrap();
        assert!(text.contains("  1000:"));
        assert!(text.contains("    1001: \"DLC\""));
    }

    #[test]
    fn adds_token_and_manifest() {
        let (directory, config) = config_with(SAMPLE);
        assert!(config.add_token(1234, "token").unwrap());
        assert!(config.set_manifest(2483190, 111).unwrap());
        let text = fs::read_to_string(directory.path().join("config.yaml")).unwrap();
        assert!(text.contains("  1234: \"token\""));
        assert!(text.contains("  2483190: 111"));
        assert!(!config.add_token(1234, "other").unwrap());
        assert!(!config.set_manifest(2483190, 111).unwrap());
    }

    #[test]
    fn writes_a_backup_before_changes() {
        let (directory, config) = config_with(SAMPLE);
        config.add_app(1).unwrap();
        assert!(directory.path().join("config.yaml.bak").exists());
    }
}
