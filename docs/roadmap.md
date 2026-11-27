# GameHaven 路线图 · 从「清单下载器」到「Linux 版 Steam 工具箱」

> 编写日期：2026-11-27
> 对照参考：SFF / SteaMidra 6.6.6（Python，Windows 优先 + Linux 补丁式支持）
> 目标定位：Linux GUI 版的 OpenSteamTool

---

## 结论

**下一步不是继续加下载功能，而是把「添加游戏 → Steam 认账 → Steam 自己下载」这条闭环做进 GUI。**

这是「Linux 版 OST」和「清单下载器」的分水岭。现有代码已经证明你能解析清单、驱动 DepotDownloader、识别 Steam 库；接下来要把这些零件换成一条用户点两下就能走完的路：

```
输入 AppID / 游戏名
   → 拉 Steam appinfo（名称、DLC、平台）
   → 写入 SLSsteam 配置（AdditionalApps / DlcData / AppTokens）
   → 让 Steam 以注入方式启动
   → 库里出现「安装」按钮，Steam 原生下载并解密
```

配套的第一件事是**环境体检**：你机器上的 SLSsteam 现在是坏的（见下方实测），而这类问题会成为用户 80% 的支持请求来源。

---

## 1. 定位：对标 OST 的正确含义

OpenSteamTool 是 Windows 上的客户端注入（`dwmapi.dll` / `xinput1_4.dll` 劫持，加载 `OpenSteamTool.dll`），**它的二进制在 Linux 上没有任何意义**，只能对标功能。

Linux 侧真正可用的对应实现：

| OST 能力 | Windows 实现 | Linux 对应 | GameHaven 是否做 |
| --- | --- | --- | --- |
| 解锁未拥有游戏 / DLC | OpenSteamTool.dll 注入 | **SLSsteam**（`LD_AUDIT` 注入 `steamclient.so`） | ✅ 核心 |
| 从 Lua 加载 depot 密钥 | SteamTools Lua | SLSsteam `config.yaml` + `stplug-in/*.lua`（兼容层） | ✅（以 YAML 为主） |
| 清单 / 版本锁定 | 内置 manifest 服务 | `ManifestIds` 覆盖 + `depotcache` 落盘 | ✅ |
| 访问令牌（保护游戏） | AppTicket | SLSsteam `AppTokens` | ✅ |
| 成就 / 统计 | 进程内 protobuf 改写 | **SLScheevo** | ✅ 第二阶段 |
| 家庭共享绕过 | LumaCore | SLSsteam `DisableFamilyShareLock` | ✅ 顺手就有 |
| 在线修复 | `-onlinefix` 参数 | SLSsteam `FakeAppIds` + `steamnetsock-patch` / `eos-proxy` | ⚠️ 可选 |
| 库存挂卡 / 市场 / 交易 | 内置 | 用 ASF，无 GUI 价值 | ❌ 不做 |
| Denuvo 授权缓存 | 凭据文件 | Linux 无对应机制 | ❌ 不做 |

**产品判断：**GameHaven 不做「下载器」，做**Steam 库管理器**。下载交给 Steam 客户端本身（它已经有 CDN、校验、断点续传、Proton 集成），GameHaven 负责「让 Steam 认为该游戏属于你 + 该装哪个版本 + 装在哪 + 怎么管理」。

与 SFF 的差异不在功能数量，而在：

- 纯 Rust、原生控件、无 WebView、无 Python 运行时；
- 所有写操作可查看、可备份、可一键撤销（SFF 到处写文件，出问题只能靠日志猜）；
- 只保留 Linux 上真实有效的链路，不背 Windows 兼容包袱；
- 数据源可配置、可降级，不把某个 24 小时过期的 API key 当唯一路径。

---

## 2. 现状盘点

### 2.1 代码现状（1,629 行）

| 模块 | 状态 | 问题 |
| --- | --- | --- |
| `steam_library.rs` | ✅ 可用 | 原生 + Flatpak 多库发现，已实现较好 |
| `steam_store.rs` | ✅ 可用 | 只查商店名称/平台，没接 appinfo |
| `depot.rs` | ⚠️ 能跑 | 依赖外部 `smd` + `DepotDownloader`（后者要 .NET 9） |
| `main.rs` | ⚠️ 需重构 | 667 行单文件，UI 与业务混在一起 |
| `docs/` | ❌ 已清空 | 需要重写 |

### 2.2 本机实测（2026-11-27，你的机器）

这是最重要的一段，直接决定下一步做什么：

| 检查项 | 结果 | 含义 |
| --- | --- | --- |
| `~/.local/share/SLSsteam/SLSsteam.so` | 26 MB，存在 | 主体文件在 |
| `~/.local/share/SLSsteam/library-inject.so` | **0 字节** | 无法 `dlopen`，注入必失败 |
| Steam 根目录下的 SLSsteam `.so` | **不存在** | 没有备用注入路径 |
| `~/.local/share/Steam/steam.sh` | **无 `LD_AUDIT`** | 启动注入未生效 |
| `~/.local/share/Steam/steam.cfg` | 存在 | 只挡住了自动更新 |
| SLSsteam 版本 | `20261014151919` | 最新为 `20261112130346`（落后一个月） |
| `config/stplug-in/2483190.lua` | 存在（SFF 写） | 没有注入层消费，等于摆设 |
| `appmanifest_2483190.acf` | `InstalledDepots` 空、`SizeOnDisk=0`、`depotcache/` 空 | Forza 安装链路中断在中途 |

**结论：**你当前 Steam 里的 Forza Horizon 6 是「库里看得见、下载走不通」的半成品状态。这不是你的代码问题，而是**环境层没有可观测性**——SLSsteam 装了没装、注入成没成、写没写对，全靠肉眼。所以 P0 第一项必须是体检。

---

## 3. 目标架构

```
src/
├── main.rs              # 只做窗口和路由
├── settings.rs          # 持久化（toml，~/.config/gamehaven/config.toml）
├── journal.rs           # 写操作日志，支持撤销
├── jobs.rs              # 统一长任务：进度、取消、事件回传
├── steam/
│   ├── env.rs           # Steam 根、库、用户、进程、注入状态
│   ├── library.rs       # 现有 steam_library.rs 迁入
│   ├── appinfo.rs       # appdetails + appinfo（名称/DLC/depot 关系）
│   ├── acf.rs           # appmanifest 读写
│   ├── lua.rs           # .lua 解析 / 生成
│   ├── manifest.rs      # manifest 下载、解密、落盘
│   └── process.rs       # 退出 / 带 LD_AUDIT 启动 / 注入校验
├── sls/
│   ├── config.rs        # config.yaml 行编辑（保留注释）
│   └── install.rs       # 版本检查、安装状态、路径定位
└── ui/
    ├── library.rs       # 库页（游戏卡 + 详情）
    ├── add.rs           # 添加游戏向导
    ├── tools.rs         # 体检 / 修复 / 重启
    └── settings.rs
```

数据流：`UI Action → Job → Domain 模块 → 文件系统/进程 → Event → UI`。禁止在 `ui` 里直接写文件或起进程。

---

## 4. 路线图

### P0 · 打通「添加游戏」闭环（约 7–10 个工作日）

| 任务 | 交付物 | 验收标准 |
| --- | --- | --- |
| P0-1 环境体检 | `steam/env.rs` + 体检面板 | 6 项检查与手工命令结果一致；本机红色项能定位到 0 字节 `library-inject.so` |
| P0-2 SLSsteam 配置编辑 | `sls/config.rs` | 对真实 `config.yaml` 增删 app，`diff` 只变目标行；幂等；写前自动备份 |
| P0-3 注入启停 | `steam/process.rs` | 重启后 `/proc/<pid>/maps` 含 `SLSsteam.so`，UI 显示「已注入」 |
| P0-4 添加游戏向导 | `ui/add.rs` + `steam/appinfo.rs` | 输入未拥有 AppID → 重启 → Steam 库出现「安装」→ 能开始下载 |
| P0-5 失败可回滚 | `journal.rs` | 每个写操作有记录，一键撤销可恢复原状 |

**验收用真实样本：**先用你库里现成的 `2483190`（Forza Horizon 6，ACF 已存在但下载中断）验证修复流程，再用一个从未拥有的 AppID 验证全新增流程。

### P1 · 清单与版本（约 7–10 个工作日）

- manifest 下载 + 解密 + 写 `depotcache`（AES-128-CBC 文件名解密、protobuf 校验）；
- `ManifestIds` 版本锁定，支持「最新版 / 指定 build」；
- DLC depot 映射（从 appinfo 的 `dlcappid` 生成 `DlcData`）；
- 游戏搜索（Store 搜索 + 可选清单库数据源，做成可插拔 provider）；
- 移除游戏（清 ACF / manifest / 配置项 / 快捷方式）。

### P2 · 库与账号（约 2–3 周）

- 库页：封面、大小、最后更新、是否注入、能否下载；
- ACF 修复 / 重建工具；
- 账号切换（`loginusers.vdf` + `AutoLoginUser`，含安全确认）；
- 成就（SLScheevo 集成，可选安装）。

### P3 · 打磨

设置页、i18n、日志面板、AppImage / Flatpak 打包、自更新、崩溃恢复。

---

## 5. P0 任务分解（可直接开工）

### P0-1 `steam/env.rs`

```rust
pub struct Environment {
    pub roots: Vec<SteamRoot>,        // native + flatpak + snap
    pub libraries: Vec<Library>,
    pub active_user: Option<SteamUser>,
    pub process: Option<SteamProcess>, // pid + 是否已注入
    pub slssteam: SlsStatus {
        installed: bool,
        version: Option<String>,
        main_so: Option<PathBuf>,
        inject_so: Option<PathBuf>,   // 0 字节要判为异常
        config_path: Option<PathBuf>,
        steam_cfg: bool,
        injected: bool,
    },
}
```

检查项（每项都要有「为什么」和「怎么办」文案）：

1. Steam 根目录是否找到（原生 / Flatpak / Snap）；
2. `steam.sh` 是否可执行、是否含 `LD_AUDIT`；
3. SLSsteam 两个 `.so` 是否存在且非空；
4. SLSsteam 版本是否落后于最新 release（GitHub API）；
5. `config.yaml` 是否存在、是否可解析、`AdditionalApps` 条数；
6. Steam 进程是否在跑、是否已注入（读 `/proc/<pid>/maps`）。

### P0-2 `sls/config.rs`

**不要用 `serde_yaml` 整体反序列化再写回**——会把用户的注释和排版全部冲掉，这是 SFF 反复出问题的地方。按行编辑：

```rust
pub fn add_additional_app(path: &Path, app_id: u32, comment: &str) -> Result<bool>;
pub fn remove_additional_app(path: &Path, app_id: u32) -> Result<bool>;
pub fn add_dlc_data(path: &Path, parent: u32, dlc: u32, name: &str) -> Result<bool>;
pub fn add_app_token(path: &Path, app_id: u32, token: &str) -> Result<bool>;
pub fn set_manifest_id(path: &Path, depot: u32, manifest: u64) -> Result<bool>;
```

规则：先备份、再原子写、写入前做 YAML 缩进合法性检查、每步返回「是否真的改了」。

### P0-3 `steam/process.rs`

- 退出：`SIGTERM` 给主进程，等待子进程退出，超时才 `SIGKILL`；
- 启动：清掉 `LD_LIBRARY_PATH` / `LD_PRELOAD` / `LD_AUDIT` 再设 `LD_AUDIT=library-inject.so:SLSsteam.so`；
- 校验：启动 10 秒后扫 `/proc/*/maps`，找 `SLSsteam.so`，失败要明确提示「装了但没注入」而不是「启动失败」。

### P0-4 `ui/add.rs`

向导四步：输入 AppID → 预览（名称、DLC 数、平台、是否已拥有）→ 确认写入 → 重启 Steam。每一步都显示「将要修改哪些文件、改什么」，这是与 SFF 最大的体验差异。

---

## 6. 从 SFF 抄什么，不抄什么

**值得抄（语义层面）**

- SLSsteam `config.yaml` 字段含义：`AppIds` / `AdditionalApps` / `DlcData` / `AppTokens` / `ManifestIds` / `FakeAppIds` / `LaunchOptions` / `IdleStatus`；
- `acf_writer.py` 的 ACF 模板（含 `StateFlags=4`、`chmod 444`）；
- `linux_download.py` 里「先写配置、再放 manifest、再写 ACF」的顺序；
- `steam_process.py` 的 `LD_AUDIT` 启动方式与 `/proc/maps` 校验思路；
- 路径探测列表：`~/.steam/root`、`~/.steam/steam`、`~/.local/share/Steam`、Flatpak `.var/app/...`、Snap；
- lua 语法：`addappid(id)`、`addappid(id, 0, key)`、`setManifestid(id, gid)`、`addtoken(id, token)`。

**不要抄**

- 60+ 个 Python 模块互相 import 的组织方式；
- PyQt5 + QWebEngine 双 UI；
- ManifestHub 24 小时 key、Hubcap key 这类外部依赖作为主路径；
- headcrab + 7z 的安装器链路（Rust 里用 `sevenz-rust` 或直接给用户手动放置）；
- `game_specific.py` 那种 35 KB 的特判集合；
- Windows 分支逻辑（写 ACF 在 Windows 被跳过的那些判断）。

---

## 7. Linux 特有坑（提前写进设计）

1. **Steam 自动更新覆盖 `steam.sh`** → 注入失效。体检里必须每次启动检查，并提供「重新注入」按钮。
2. **ACF 权限 444 + Steam 运行中会被重写** → 写 ACF 前必须退出 Steam，写完再启动。
3. **Flatpak 沙箱** → 注入发生在沙箱内，路径是 `.var/app/...`，`LD_AUDIT` 要指向沙箱可见路径。
4. **Steam Deck Game Mode 不支持注入** → UI 明确提示切桌面模式。
5. **manifest 镜像不稳定**（403、格式变更、限流）→ 多来源 + 超时 + 明确降级提示，不要静默失败。
6. **SLSsteam 迭代很快**（10/14 → 11/12）→ 版本检查 + 兼容性提示，不要硬编码行为。
7. **lua 与 config.yaml 的关系**：SLSsteam 主流配置在 `config.yaml`，`stplug-in/*.lua` 是 SteamTools 兼容生态（LuaTools 系）。GameHaven 以 YAML 为唯一写入源，lua 只在导入外部清单时生成。
8. **不要 sudo**：一切写在用户目录，避免权限污染。

---

## 8. 本周动作（按顺序）

- [ ] `.gitignore` 加 `reverse-skill/`、`.qoder/`（这两个是你机器上的临时目录，不该进仓库）
- [ ] 新建 `docs/roadmap.md`（本文档）并重写 `README.md` 的定位段
- [ ] 实现 `steam/env.rs`，先做只读体检面板，跑通本机 6 项检查
- [ ] 实现 `sls/config.rs` 的读 + 幂等写，用你现有的 `config.yaml` 做 diff 验证
- [ ] 实现 `steam/process.rs` 的重启 + 注入校验，修好本机 SLSsteam
- [ ] 用 `2483190` 走一遍「体检 → 修复 → 重启 → 下载」，记录每一步实际写入了什么
- [ ] 再拿一个从未拥有的 AppID 走全新增流程

**判断标准：**当你能在这台机器上，从零启动 GameHaven，点三下就让一个未拥有的游戏出现在 Steam 库里并开始下载时，P0 就算完成，再去碰清单和版本。
