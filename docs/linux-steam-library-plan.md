# 游栖（GameHaven）— Linux Steam 清单与游戏库工具计划

GameHaven 自建 UI。后端只接可独立运行的 CLI 或有明确调用接口的 Linux 共享库。首版选用独立 CLI，通过子进程参数、标准输出、标准错误和退出码通信。

GUI 已选定为 `eframe + egui 0.37`，使用纯 Rust 原生窗口和 `wgpu` 渲染，不使用 Tauri、WebView、GTK 或 Qt。具体比较和 Linux 验证边界见 [GUI 选型](gui-selection.md)。

## Steam 商店查询

在原型输入 AppID 并点击「查询 Steam 商店」，即可查看公开名称和商店列出的平台。界面仅在点击查询时把 AppID 发送给 Steam 商店，查询在后台执行，单次超时设为 12 秒。商店平台字段用于显示商品信息，安装仍需通过 Steam 许可证和下载流程。[商店响应示例](https://store.steampowered.com/api/appdetails?appids=440&l=schinese)

## 已验证的 Linux CLI

选用候选项目的具体版本是 `mcbabel/Steam-Manifest-Downloader v1.5.0`。该版本同时发布了 Linux x64 终端程序 `Steam-Manifest-Downloader-Terminal_1.5.0_linux-x64` 和 amd64 桌面 AppImage。GameHaven 只调用终端程序 `smd`。这个 release 的资产列表中没有独立发布的后端 `.so`。[v1.5.0 发布页](https://github.com/mcbabel/Steam-Manifest-Downloader/releases/tag/v1.5.0)

已在本机 Linux x86_64 核对发布资产的 SHA-256，并在断网、临时数据目录、无 TTY 的环境下执行发布版二进制。验证结果如下。

| 用例 | 标准输出 | 标准错误 | 退出码 |
| --- | --- | --- | --- |
| `smd --version` | `smd 1.5.0` | 空 | 0 |
| `smd --data-dir /tmp/data history --json` | `[]` | 空 | 0 |
| `smd history --invalid-option` | 空 | 包含无效参数名和用法 | 2 |

以上覆盖进程启动、空历史 JSON 和参数错误。在线查询、真实清单处理、下载完成和取消仍待验收。该版本源码提供无界面的查询和下载子命令，具体输出按子命令分别验证。[v1.5.0 CLI 入口](https://github.com/mcbabel/Steam-Manifest-Downloader/blob/v1.5.0/src-tui/src/main.rs)

## 实施顺序

1. **验证业务接口。** 用一个有权访问的 Steam 应用测试查询、已有清单输入和安装流程，记录各步实际返回的数据、进度、错误和取消结果。另测损坏清单。如果需要按当前账号权限下载指定 depot，评估官方上游 SteamRE 的 DepotDownloader CLI。[DepotDownloader](https://github.com/SteamRE/DepotDownloader)
2. **自建 UI。** 提供 AppID 输入、本地清单选择和目标库选择。用参数数组启动后端，将已验证的结构化输出映射为任务状态。清单解析、文件下载、交给 Steam 安装分别展示，账号的下载权限由 Steam 许可证决定。[Steamworks 许可证说明](https://partner.steamgames.com/doc/store/application/packages)
3. **添加本地游戏。** 先用临时 Steam 配置验证候选 CLI 或共享库能按外部传入的游戏名称、启动文件和目录写入快捷方式，再接 UI。BoilR 仅作功能参考，其 Rust crate 不计为已验收的共享库。验收包括原生和 Flatpak Steam 的路径发现、重启后保留、重复导入不产生重复条目，以及用一款已有的 DRM-free 游戏通过 Proton 启动。[BoilR](https://github.com/PhilipK/BoilR)

Denuvo 跨账号授权仍是独立的待研究需求。当前没有已验证的 Linux CLI 或共享库可供集成。
