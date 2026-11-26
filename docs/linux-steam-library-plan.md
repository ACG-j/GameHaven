# 游栖（GameHaven）— Linux Steam 清单与游戏库工具计划

GameHaven 自建 UI。后端只接可独立运行的 CLI 或有明确调用接口的 Linux 共享库。首版选用独立 CLI，通过子进程参数、标准输出、标准错误和退出码通信。

GUI 已选定为 `eframe + egui 0.37`，使用纯 Rust 原生窗口和 `wgpu` 渲染，不使用 Tauri、WebView、GTK 或 Qt。具体比较和 Linux 验证边界见 [GUI 选型](gui-selection.md)。

## Steam 商店查询

在原型输入 AppID 并点击「查询 Steam 商店」，即可查看公开名称和商店列出的平台。界面仅在点击查询时把 AppID 发送给 Steam 商店，查询在后台执行，单次超时设为 12 秒。商店平台字段用于显示商品信息，安装仍需通过 Steam 许可证和下载流程。[商店响应示例](https://store.steampowered.com/api/appdetails?appids=440&l=schinese)

## 已验证的 Linux CLI

清单预览使用 `mcbabel/Steam-Manifest-Downloader v1.5.0` 的 Linux x64 终端程序 `smd`。下载使用 `SteamRE/DepotDownloader 3.4.0`，由 Steam 匿名会话决定可访问的 Depot。两个工具需单独安装，并放在 `PATH` 中。也可通过 `GAMEHAVEN_SMD` 和 `GAMEHAVEN_DEPOT_DOWNLOADER` 指定可执行文件的绝对路径。[smd 发布页](https://github.com/mcbabel/Steam-Manifest-Downloader/releases/tag/v1.5.0) · [DepotDownloader 发布页](https://github.com/SteamRE/DepotDownloader/releases/tag/DepotDownloader_3.4.0)

已在本机 Linux x86_64 核对发布资产的 SHA-256，并在断网、临时数据目录、无 TTY 的环境下执行发布版二进制。验证结果如下。

| 用例 | 标准输出 | 标准错误 | 退出码 |
| --- | --- | --- | --- |
| `smd --version` | `smd 1.5.0` | 空 | 0 |
| `smd --data-dir /tmp/data history --json` | `[]` | 空 | 0 |
| `smd history --invalid-option` | 空 | 包含无效参数名和用法 | 2 |

`smd download --list --json` 已用 `.lua` 和 `.st` 样本验证。有效清单输出含 AppID、Depot ID 和 Manifest ID 的 `plan`，损坏清单退出码为 1。GameHaven 使用临时数据目录调用 `smd`，清单预览可取消。[v1.5.0 CLI 入口](https://github.com/mcbabel/Steam-Manifest-Downloader/blob/v1.5.0/src-tui/src/cli.rs)

每次下载选择一个 Depot，写入目标库下的新目录。进度来自 CLI 的百分比行，成功状态要求退出码为 0 且 CLI 汇报完成了一个 Depot。取消后保留已写入的文件，重新下载会创建新目录。下载工具继承启动 GameHaven 时的 `HTTP_PROXY` 和 `HTTPS_PROXY`。本机直连 Steam3 超时，使用已有的 HTTP 代理后连通。

2026-11-26 在 X11 下验证了有效和损坏清单、真实下载与取消。TF2 Dedicated Server 的 AppID `232250`、Depot `232257` 和 Manifest `4797708003880603728` 经 GameHaven 下载成功，产出两个文件，共 9,989 字节。测试进程另覆盖了进度显示及退出码为 0 但无完成回执的情况。[DepotDownloader 3.4.0 下载实现](https://github.com/SteamRE/DepotDownloader/blob/DepotDownloader_3.4.0/DepotDownloader/ContentDownloader.cs)

## Steam 库与下载文件复用

GameHaven 可识别原生与 Flatpak 的 Steam 配置、外部库和已安装应用，也可从已发现的库选择 Depot 下载目录。Steam 客户端安装按钮发出 `steam://install/<AppID>` 请求，由客户端处理许可证与安装位置。已用替代启动器验证请求参数，并用 SteamCMD 在临时目录真实安装 AppID `1007`，随后验证 GameHaven 能识别其安装清单和库路径。[Steam 浏览器协议](https://developer.valvesoftware.com/wiki/Steam_browser_protocol)

Depot 下载完成后可将文件复制到指定 Steam 库的 `steamapps/common` 下。GameHaven 优先读取已有 AppID 安装记录中的 `installdir`，仅写入缺失的文件。回到同一个 Steam 库继续安装或校验，Steam 会识别已有文件并获取缺失内容。应用的安装状态和授权由 Steam 处理。[Steam 支持文档](https://help.steampowered.com/en/faqs/view/4578-18A7-C819-8620)

已在隔离 X11 会话用先前真实下载的 Depot 文件验证复制，确认已有文件和 Steam 安装记录保持原样。替代启动器捕获到了安装和校验两种请求。登录客户端后的发现文件和补齐流程待验收。

Denuvo 跨账号授权仍是独立的待研究需求。当前没有已验证的 Linux CLI 或共享库可供集成。
