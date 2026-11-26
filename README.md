# 游栖 · GameHaven

面向 Linux 的 Steam 游戏库工具。可以按 AppID 查询游戏，预览本地 Steam 清单，匿名下载单个 Depot，发起 Steam 客户端安装，或将本地游戏添加到 Steam。界面使用 Rust、egui 和 wgpu，支持 X11 和 Wayland。

## 运行

需要 Rust 1.95 或更新版本。中文界面优先使用系统安装的 Noto Sans CJK SC Regular。

```bash
cargo run --locked
```

清单预览使用 `smd` 1.5.0，匿名下载使用 `DepotDownloader` 3.4.0。将两个可执行文件放入 `PATH`，也可以指定它们的绝对路径。下载程序会继承 `HTTP_PROXY` 和 `HTTPS_PROXY`。[smd 发布页](https://github.com/mcbabel/Steam-Manifest-Downloader/releases/tag/v1.5.0) · [DepotDownloader 发布页](https://github.com/SteamRE/DepotDownloader/releases/tag/DepotDownloader_3.4.0)

```bash
GAMEHAVEN_SMD=/absolute/path/to/smd \
GAMEHAVEN_DEPOT_DOWNLOADER=/absolute/path/to/DepotDownloader \
cargo run --locked
```

## 使用

- 在「Steam 游戏」中输入 AppID 可以查询商店信息，或打开 Steam 客户端发起安装。安装完成后，点「刷新 Steam 库」查看安装位置。
- 选择 `.lua` 或 `.st` 清单并解析，选定 Depot 和目标目录后即可发起匿名下载。下载权限由 Steam 决定，已取回的文件保存在目标目录下的独立子目录中。
- 添加本地游戏时先退出 Steam，然后在「本地游戏」中选择账号、启动文件和工作目录。Windows 游戏可在 Steam 快捷方式的属性中选择 Proton。

[工具配置与验证记录](docs/linux-steam-library-plan.md)
