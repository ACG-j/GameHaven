use std::fs;
use std::path::{Path, PathBuf};

use crate::steam_library;

/// 单条体检结果的严重程度。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Ok,
    Warn,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "正常",
            Self::Warn => "注意",
            Self::Error => "异常",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientKind {
    Native,
    Flatpak,
    Snap,
}

impl ClientKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "原生",
            Self::Flatpak => "Flatpak",
            Self::Snap => "Snap",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SteamRoot {
    pub path: PathBuf,
    pub kind: ClientKind,
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: i32,
    pub name: String,
    /// `None` 表示没有权限读取 `/proc/<pid>/maps`。
    pub injected: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct SlsStatus {
    pub dir: Option<PathBuf>,
    pub main_so: Option<PathBuf>,
    pub main_so_len: u64,
    pub legacy_inject_so: Option<PathBuf>,
    pub legacy_inject_len: u64,
    pub config: Option<PathBuf>,
    pub additional_apps: Option<usize>,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Launcher {
    pub bin: bool,
    pub desktop: bool,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub hint: Option<String>,
}

impl Check {
    fn new(level: Level, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            level,
            title: title.into(),
            detail: detail.into(),
            hint: None,
        }
    }

    fn ok(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(Level::Ok, title, detail)
    }

    fn warn(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(Level::Warn, title, detail)
    }

    fn error(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(Level::Error, title, detail)
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// 一次环境体检的完整结果。
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // 原始数据留给后续「修复」流程，面板目前只渲染 checks。
pub struct Report {
    pub roots: Vec<SteamRoot>,
    pub depotcache: usize,
    pub libraries: Vec<steam_library::Library>,
    pub processes: Vec<ProcessInfo>,
    pub sls: SlsStatus,
    pub steam_cfg: Option<(PathBuf, bool)>,
    pub launcher: Launcher,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut ok = 0;
        let mut warn = 0;
        let mut error = 0;
        for check in &self.checks {
            match check.level {
                Level::Ok => ok += 1,
                Level::Warn => warn += 1,
                Level::Error => error += 1,
            }
        }
        (ok, warn, error)
    }

    /// 检查线程自身失败时用的占位报告。
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            checks: vec![Check::error("环境检查", message)],
            ..Self::default()
        }
    }
}

/// 只读地收集 Steam 与 SLSsteam 的当前状态。不会修改任何文件。
pub fn collect() -> Report {
    let roots = find_roots();
    let libraries = steam_library::discover();
    let processes = find_steam_processes();
    let sls = find_sls(&roots);
    let sls_load_error = find_sls_load_error(&roots);
    let depotcache = count_depotcache(&roots);
    let steam_cfg = find_steam_cfg(&roots);
    let launcher = find_launcher();
    let checks = build_checks(
        &roots,
        &libraries,
        &processes,
        &sls,
        sls_load_error.as_deref(),
        depotcache,
        steam_cfg.as_ref(),
        &launcher,
    );
    Report {
        roots,
        libraries,
        processes,
        sls,
        depotcache,
        steam_cfg,
        launcher,
        checks,
    }
}

fn candidate_roots() -> Vec<(PathBuf, ClientKind)> {
    let Some(home) = std::env::home_dir() else {
        return Vec::new();
    };
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    vec![
        (data.join("Steam"), ClientKind::Native),
        (home.join(".local/share/Steam"), ClientKind::Native),
        (home.join(".steam/steam"), ClientKind::Native),
        (home.join(".steam/root"), ClientKind::Native),
        (home.join(".steam/debian-installation"), ClientKind::Native),
        (
            home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
            ClientKind::Flatpak,
        ),
        (
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            ClientKind::Flatpak,
        ),
        (
            home.join(".var/app/com.valvesoftware.Steam/.steam/steam"),
            ClientKind::Flatpak,
        ),
        (
            home.join("snap/steam/common/.local/share/Steam"),
            ClientKind::Snap,
        ),
    ]
}

fn looks_like_steam(path: &Path) -> bool {
    path.join("steamapps").is_dir()
        || path.join("steam.sh").is_file()
        || path.join("ubuntu12_32").is_dir()
}

fn find_roots() -> Vec<SteamRoot> {
    let mut seen = Vec::new();
    let mut roots = Vec::new();
    for (path, kind) in candidate_roots() {
        if !looks_like_steam(&path) {
            continue;
        }
        let Ok(path) = path.canonicalize() else {
            continue;
        };
        if seen.contains(&path) {
            continue;
        }
        seen.push(path.clone());
        roots.push(SteamRoot { path, kind });
    }
    roots
}

fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

fn is_steam_process(comm: &str, cmdline: &[String]) -> bool {
    if comm == "steam" || comm == "steamwebhelper" || comm == "steam.sh" {
        return true;
    }
    cmdline.iter().any(|arg| {
        Path::new(arg)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "steam" || name == "steam.sh")
    })
}

pub(crate) fn is_injected_maps(maps: &str) -> bool {
    maps.contains("SLSsteam.so") || maps.contains("SLSteam.so") || maps.contains("library-inject.so")
}

pub(crate) fn find_steam_processes() -> Vec<ProcessInfo> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut processes = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        let Some(comm) = fs::read_to_string(format!("/proc/{pid}/comm"))
            .ok()
            .map(|line| line.trim().to_owned())
        else {
            continue;
        };
        let cmdline = fs::read(format!("/proc/{pid}/cmdline"))
            .ok()
            .map(|bytes| split_nul(&bytes))
            .unwrap_or_default();
        if !is_steam_process(&comm, &cmdline) {
            continue;
        }
        let injected = fs::read_to_string(format!("/proc/{pid}/maps"))
            .ok()
            .map(|maps| is_injected_maps(&maps));
        processes.push(ProcessInfo {
            pid,
            name: comm,
            injected,
        });
    }
    processes.sort_by_key(|process| process.pid);
    processes
}

/// 可选成就工具 SLScheevo 的位置。
pub fn slscheevo_path() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    [
        home.join(".local/share/SLScheevo"),
        home.join(".local/bin/SLScheevo"),
        PathBuf::from("/usr/bin/SLScheevo"),
        PathBuf::from("/usr/local/bin/SLScheevo"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub fn steam_root() -> Option<PathBuf> {
    find_roots().into_iter().next().map(|root| root.path)
}

pub fn sls_so_path() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    [
        home.join(".local/share/SLSsteam/SLSsteam.so"),
        home.join(".local/share/SLSteam/SLSteam.so"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/SLSsteam/SLSsteam.so"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub fn sls_config_path() -> Option<PathBuf> {
    let home = std::env::home_dir()?;
    [
        home.join(".config/SLSsteam/config.yaml"),
        home.join(".var/app/com.valvesoftware.Steam/.config/SLSsteam/config.yaml"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn find_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn find_sls(roots: &[SteamRoot]) -> SlsStatus {
    let mut status = SlsStatus::default();
    let Some(home) = std::env::home_dir() else {
        return status;
    };
    let dirs = [
        home.join(".local/share/SLSsteam"),
        home.join(".local/share/SLSteam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/SLSsteam"),
    ];
    status.dir = dirs.into_iter().find(|dir| dir.is_dir());

    let mut main_so = status
        .dir
        .as_deref()
        .and_then(|dir| find_file(dir, &["SLSsteam.so", "SLSteam.so"]));
    if main_so.is_none() {
        for root in roots {
            main_so = find_file(&root.path, &["SLSsteam.so", "SLSteam.so"]).or_else(|| {
                find_file(
                    &root.path.join("ubuntu12_32"),
                    &["SLSteam.so", "SLSsteam.so"],
                )
            });
            if main_so.is_some() {
                break;
            }
        }
    }
    if let Some(path) = &main_so {
        status.main_so_len = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    }
    status.main_so = main_so;

    let mut legacy = status
        .dir
        .as_deref()
        .and_then(|dir| find_file(dir, &["library-inject.so"]));
    if legacy.is_none() {
        if let Some(root) = roots.first() {
            legacy = find_file(&root.path, &["library-inject.so"]);
        }
    }
    if let Some(path) = &legacy {
        status.legacy_inject_len = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    }
    status.legacy_inject_so = legacy;

    status.config = [
        home.join(".config/SLSsteam/config.yaml"),
        home.join(".var/app/com.valvesoftware.Steam/.config/SLSsteam/config.yaml"),
    ]
    .into_iter()
    .find(|path| path.is_file());

    status.additional_apps = status
        .config
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|text| count_additional_apps(&text));

    status.version = [
        status.dir.as_deref().map(|dir| dir.join("VERSION")),
        Some(home.join(".local/share/SteaMidra/SLSsteam/VERSION")),
    ]
    .into_iter()
    .flatten()
    .find_map(|path| {
        fs::read_to_string(path)
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    });

    status
}

fn find_sls_load_error(roots: &[SteamRoot]) -> Option<String> {
    let text = fs::read_to_string(roots.first()?.path.join("logs/console-linux.txt")).ok()?;
    sls_load_error(&text)
}

/// 只看最近一次 Steam 启动之后的日志，避免历史错误一直报。
pub(crate) fn sls_load_error(text: &str) -> Option<String> {
    let start = text
        .rfind("steam.sh[")
        .or_else(|| text.rfind("Startup - Steam Client launched"))?;
    let tail = &text[start..];
    if tail.contains("CURL_OPENSSL_4") && tail.contains("SLSsteam") {
        Some("SLSsteam 未能加载：Steam Runtime 的 libcurl 缺少 CURL_OPENSSL_4。".to_owned())
    } else {
        None
    }
}

fn find_steam_cfg(roots: &[SteamRoot]) -> Option<(PathBuf, bool)> {
    roots.iter().find_map(|root| {
        let path = root.path.join("steam.cfg");
        let text = fs::read_to_string(&path).ok()?;
        Some((path, text.contains("BootStrapperInhibitAll")))
    })
}

fn find_launcher() -> Launcher {
    let Some(home) = std::env::home_dir() else {
        return Launcher::default();
    };
    let injected = |path: &Path| {
        fs::read_to_string(path)
            .map(|text| text.contains("LD_AUDIT") && text.contains("SLSsteam"))
            .unwrap_or(false)
    };
    Launcher {
        bin: injected(&home.join(".local/bin/steam")),
        desktop: injected(&home.join(".local/share/applications/steam.desktop")),
    }
}

/// 统计 `config.yaml` 中 `AdditionalApps` 段的 AppID 条数。
pub(crate) fn count_additional_apps(yaml: &str) -> usize {
    let mut inside = false;
    let mut count = 0;
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("AdditionalApps:") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        let Some(rest) = trimmed.strip_prefix('-') else {
            continue;
        };
        let value = rest.trim().split_whitespace().next().unwrap_or("");
        if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
            count += 1;
        }
    }
    count
}

fn root_list(roots: &[SteamRoot]) -> String {
    let mut parts: Vec<_> = roots
        .iter()
        .take(3)
        .map(|root| format!("{} · {}", root.kind.label(), root.path.display()))
        .collect();
    if roots.len() > 3 {
        parts.push(format!("等 {} 个", roots.len()));
    }
    parts.join("；")
}

fn library_list(libraries: &[steam_library::Library]) -> String {
    let mut parts: Vec<_> = libraries
        .iter()
        .take(3)
        .map(|library| format!("{} · {}", library.client.name(), library.path.display()))
        .collect();
    if libraries.len() > 3 {
        parts.push(format!("等 {} 个", libraries.len()));
    }
    parts.join("；")
}

fn count_depotcache(roots: &[SteamRoot]) -> usize {
    roots
        .iter()
        .map(|root| {
            fs::read_dir(root.path.join("depotcache"))
                .map(|entries| {
                    entries
                        .flatten()
                        .filter(|entry| {
                            entry
                                .file_name()
                                .to_string_lossy()
                                .ends_with(".manifest")
                        })
                        .count()
                })
                .unwrap_or(0)
        })
        .sum()
}

fn build_checks(
    roots: &[SteamRoot],
    libraries: &[steam_library::Library],
    processes: &[ProcessInfo],
    sls: &SlsStatus,
    sls_load_error: Option<&str>,
    depotcache: usize,
    steam_cfg: Option<&(PathBuf, bool)>,
    launcher: &Launcher,
) -> Vec<Check> {
    let mut checks = Vec::new();

    if roots.is_empty() {
        checks.push(
            Check::error("Steam 安装", "没有找到 Steam 安装目录。")
                .with_hint("先安装并登录一次 Steam 客户端，再重新检查。"),
        );
    } else {
        checks.push(Check::ok(
            "Steam 安装",
            format!("发现 {} 个安装：{}", roots.len(), root_list(roots)),
        ));
    }

    if libraries.is_empty() {
        checks.push(
            Check::warn("Steam 库", "没有发现可用的 Steam 游戏库。")
                .with_hint("启动 Steam 并完成初始化，或在 Steam 中添加库文件夹。"),
        );
    } else {
        checks.push(Check::ok(
            "Steam 库",
            format!(
                "发现 {} 个游戏库：{}",
                libraries.len(),
                library_list(libraries)
            ),
        ));
    }

    match (&sls.main_so, sls.main_so_len) {
        (None, _) => checks.push(
            Check::error("SLSsteam 主库", "没有找到 SLSsteam.so。")
                .with_hint("安装最新版 SLSsteam 后重新检查。"),
        ),
        (Some(path), 0) => checks.push(
            Check::error(
                "SLSsteam 主库",
                format!("{} 是 0 字节，安装已损坏。", path.display()),
            )
            .with_hint("重新解压 SLSsteam release，替换这个文件。"),
        ),
        (Some(path), len) => {
            let version = sls.version.as_deref().unwrap_or("版本未知");
            checks.push(Check::ok(
                "SLSsteam 主库",
                format!(
                    "{version} · {:.1} MB · {}",
                    len as f64 / 1_048_576.0,
                    path.display()
                ),
            ));
        }
    }

    match (&sls.legacy_inject_so, sls.legacy_inject_len) {
        (None, _) => checks.push(Check::ok("兼容文件", "没有旧版 library-inject.so。")),
        (Some(path), 0) => checks.push(
            Check::warn(
                "兼容文件",
                format!("{} 是 0 字节残留。", path.display()),
            )
            .with_hint("新版 SLSsteam 只需要 SLSsteam.so，可以删除这个文件。"),
        ),
        (Some(path), _) => checks.push(
            Check::warn(
                "兼容文件",
                format!("检测到旧版注入库 {}。", path.display()),
            )
            .with_hint("旧版双 so 注入方式，建议清理，避免和新版混用。"),
        ),
    }

    match (&sls.config, sls.additional_apps) {
        (Some(path), Some(apps)) => checks.push(Check::ok(
            "SLSsteam 配置",
            format!("{} · AdditionalApps {apps} 条", path.display()),
        )),
        (Some(path), None) => checks.push(Check::warn(
            "SLSsteam 配置",
            format!("{} 无法读取。", path.display()),
        )),
        (None, _) => checks.push(
            Check::warn("SLSsteam 配置", "没有找到 config.yaml。")
                .with_hint("启动一次 Steam，让 SLSsteam 生成默认配置。"),
        ),
    }

    // SLSsteam 的 64 位构建需要 64 位 Steam 客户端；这个标记会把 Steam 锁在 32 位。
    let locked_32 = roots
        .first()
        .is_some_and(|root| root.path.join(".steam-disable-steamrt64-client").exists());
    let steamrt64 = roots
        .first()
        .is_some_and(|root| root.path.join("steamrt64/steam").is_file());
    if locked_32 {
        checks.push(
            Check::warn(
                "Steam 客户端",
                "存在 .steam-disable-steamrt64-client，Steam 被锁定在 32 位客户端。",
            )
            .with_hint(
                "SLSsteam 的 64 位构建需要 64 位客户端；删除这个标记后重启 Steam。",
            ),
        );
    } else if steamrt64 {
        checks.push(Check::ok("Steam 客户端", "使用 SteamRT3 64 位客户端。"));
    } else {
        checks.push(
            Check::warn("Steam 客户端", "没有找到 steamrt64/steam。")
                .with_hint("更新 Steam 客户端后再试。"),
        );
    }

    match sls_load_error {
        Some(message) => checks.push(
            Check::error("SLSsteam 加载", message).with_hint(
                "把 ~/.local/share/Steam/ubuntu12_32/steam-runtime/pinned_libs_64/libcurl.so.4 \
                 指向系统 /usr/lib/libcurl.so.4，然后重启 Steam。",
            ),
        ),
        None => checks.push(Check::ok(
            "SLSsteam 加载",
            "最近一次 Steam 启动没有 SLSsteam 加载错误。",
        )),
    }

    if processes.is_empty() {
        checks.push(
            Check::warn("注入状态", "Steam 未运行。")
                .with_hint("从桌面图标或 ~/.local/bin/steam 启动后会自动注入。"),
        );
    } else if let Some(process) = processes
        .iter()
        .find(|process| process.injected == Some(true))
    {
        checks.push(Check::ok(
            "注入状态",
            format!(
                "已注入（PID {} · {}，共 {} 个 Steam 进程）。",
                process.pid,
                process.name,
                processes.len()
            ),
        ));
    } else if processes
        .iter()
        .all(|process| process.injected == Some(false))
    {
        checks.push(
            Check::error(
                "注入状态",
                format!(
                    "Steam 正在运行，但 {} 个进程都没有加载 SLSsteam。",
                    processes.len()
                ),
            )
            .with_hint("退出 Steam，再从桌面图标或 ~/.local/bin/steam 启动。"),
        );
    } else {
        checks.push(
            Check::warn("注入状态", "无法读取 Steam 进程的内存映射。")
                .with_hint("确认 GameHaven 和 Steam 属于同一个用户。"),
        );
    }

    if launcher.bin || launcher.desktop {
        let mut parts = Vec::new();
        if launcher.bin {
            parts.push("终端 wrapper");
        }
        if launcher.desktop {
            parts.push("桌面图标");
        }
        checks.push(Check::ok(
            "启动入口",
            format!("已配置注入：{}。", parts.join("、")),
        ));
    } else {
        checks.push(
            Check::warn("启动入口", "桌面图标和终端启动都没有配置 LD_AUDIT。")
                .with_hint("给启动入口加注入，否则 Steam 不会加载 SLSsteam。"),
        );
    }

    if depotcache > 0 {
        checks.push(Check::ok(
            "本地 manifest",
            format!("depotcache 里有 {depotcache} 个 manifest。"),
        ));
    } else {
        checks.push(
            Check::warn("本地 manifest", "depotcache 是空的。")
                .with_hint("Steam 会从 CDN 拉取最新版；需要锁定版本时，在添加页导入 .manifest。"),
        );
    }

    match steam_cfg {
        Some((path, true)) => checks.push(Check::ok(
            "更新抑制",
            format!("{} 已阻止 Steam 自更新。", path.display()),
        )),
        Some((path, false)) => checks.push(
            Check::warn(
                "更新抑制",
                format!("{} 存在，但没有 BootStrapperInhibitAll。", path.display()),
            )
            .with_hint("写入 BootStrapperInhibitAll=enable。"),
        ),
        None => checks.push(
            Check::warn("更新抑制", "没有 steam.cfg。")
                .with_hint("写入 BootStrapperInhibitAll=enable，避免 Steam 更新后注入失效。"),
        ),
    }

    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_additional_apps_entries() {
        let yaml = "AppIds:\n  - 440\nAdditionalApps:\n  - 1493710   # migrated\n  - 228980\n  - abc\nAppTokens:\n  2483190: \"1\"\n";
        assert_eq!(count_additional_apps(yaml), 2);
    }

    #[test]
    fn ignores_inline_empty_additional_apps() {
        let yaml = "AdditionalApps:\nAppIds:\n  - 440\n";
        assert_eq!(count_additional_apps(yaml), 0);
    }

    #[test]
    fn finds_sls_load_error_only_in_latest_start() {
        let old = "steam.sh[1]: boot\nERROR: CURL_OPENSSL_4 not found (required by SLSsteam.so)";
        assert!(sls_load_error(old).is_some());
        let recovered = "steam.sh[1]: boot\nERROR: CURL_OPENSSL_4 not found\nsteam.sh[2]: boot\nLog already open";
        assert!(sls_load_error(recovered).is_none());
    }

    #[test]
    fn detects_injected_maps() {
        assert!(is_injected_maps(
            "7f00-7f10 r-xp 00000000 08:01 123 /home/x/.local/share/SLSsteam/SLSsteam.so\n"
        ));
        assert!(!is_injected_maps(
            "7f00-7f10 r-xp 00000000 08:01 123 /usr/lib/steam/steam\n"
        ));
    }

    #[test]
    #[ignore = "manual smoke test"]
    fn dumps_environment_report() {
        let report = collect();
        let (ok, warn, error) = report.counts();
        println!("checks: {ok} ok / {warn} warn / {error} error");
        for check in &report.checks {
            println!("[{}] {} - {}", check.level.label(), check.title, check.detail);
            if let Some(hint) = &check.hint {
                println!("    hint: {hint}");
            }
        }
    }

    #[test]
    fn recognises_steam_processes() {
        assert!(is_steam_process("steam", &[]));
        assert!(is_steam_process(
            "bash",
            &["/home/x/.local/share/Steam/steam.sh".into()]
        ));
        assert!(!is_steam_process("firefox", &["/usr/lib/firefox/firefox".into()]));
    }
}
