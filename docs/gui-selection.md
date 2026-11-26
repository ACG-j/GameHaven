# GameHaven GUI 选型

核查日期：2026-11-26

## 结论

首版采用 `eframe 0.37.0 + egui 0.37.0`。

这条路线满足项目的纯 Rust 约束。应用代码、窗口事件和控件绘制都在 Rust 中完成，不嵌入 HTML、JavaScript 或 WebView。`eframe` 使用 `winit` 创建原生窗口，默认渲染器是 `wgpu`，Linux 通过 Cargo feature 同时支持 X11 和 Wayland。`accesskit`、剪贴板、系统字体和持久化能力也有现成的 feature，可以按产品需求启用。

选择依据如下。

- 当前稳定版本为 `0.37.0`，仓库在核查日仍有提交，许可证为 MIT 或 Apache-2.0。
- `eframe` 的默认 feature 已包含 `wayland`、`x11`、`wgpu` 和 `accesskit`，适合当前 Linux 桌面目标。
- egui 的即时模式适合游戏库、下载任务和状态面板这类频繁变化的界面，UI 状态可以保留在应用状态模型中，后台 CLI 任务通过 channel 回传事件。
- 不需要引入 GTK、Qt 或浏览器运行时。窗口系统和图形驱动仍然属于 Linux 桌面程序的系统依赖，这是原生窗口本身的边界。

参考资料：[`eframe 0.37.0 Cargo.toml`](https://github.com/emilk/egui/blob/0.37.0/crates/eframe/Cargo.toml)、[`egui 0.37.0 release`](https://github.com/emilk/egui/releases/tag/0.37.0)

## 候选比较

| 框架 | 结论 | 原因 |
| --- | --- | --- |
| `eframe + egui 0.37.0` | 采用 | 纯 Rust、原生窗口、Linux X11/Wayland、`wgpu` 和 AccessKit 支持完整，当前版本成熟度适合作为首版基础。 |
| `iced 0.14.0` | 备选 | 纯 Rust，提供 `wgpu` 和 `tiny-skia`，也支持 X11/Wayland。上游 README 仍将软件标为 experimental，因此暂不作为首版基础。 |
| `Slint 1.19.1` | 排除 | 原生渲染和 Rust 集成可用，但 UI 需要 `.slint` 声明文件，项目会增加第二套 UI 语言，不符合本项目严格的纯 Rust 源码约束。 |
| `GPUI 0.2.2` | 排除 | GPU 框架本身是 Rust，但 crate 仍处于 0.x，主要随 Zed 仓库演进，作为独立桌面产品的 API 风险较高。 |
| `Makepad` | 排除 | 可以做纯 Rust 自绘 UI，但控件和桌面应用生态较小，当前需求不需要承担额外的框架风险。 |
| `Dioxus Desktop` / `Tauri` | 排除 | 桌面实现依赖 `wry` WebView，和本项目不使用 WebView 的约束冲突。 |

候选资料：[`iced 0.14.0`](https://github.com/iced-rs/iced/releases/tag/0.14.0)、[`Slint 1.19.1`](https://github.com/slint-ui/slint/releases/tag/v1.19.1)、[`GPUI 0.2.2`](https://crates.io/crates/gpui/0.2.2)、[`Dioxus Desktop Cargo.toml`](https://github.com/DioxusLabs/dioxus/blob/main/packages/desktop/Cargo.toml)

## 实施约束

首个 GUI 原型固定使用以下边界。

1. 依赖使用 `eframe`，不引入 `tauri`、`wry`、GTK 或 Qt。
2. 视图只负责绘制和发出动作，Steam 查询、清单解析、下载进程和取消逻辑放在独立的应用状态与后台任务中。
3. 后台任务不得阻塞 egui 帧循环，任务状态通过有界 channel 或等价的事件队列更新。
4. 首个原型同时在 X11 和 Wayland 下启动，先验证窗口、字体、文件选择、剪贴板和高 DPI，再接入 Steam CLI。
5. 依赖版本先固定在 `0.37` 系列，完成原型和 Linux 打包验证后再升级小版本。

## 运行原型

在 Linux 上使用 Rust 1.95 或更新版本运行 `cargo run --locked`。界面优先使用系统安装的 Noto Sans CJK SC Regular 绘制中英文，也支持 Noto Sans SC、思源黑体等系统字体。缺失字形由 egui 继续回退。应用使用 `egui-file-dialog` 在窗口内选择清单文件和目标库目录。

原型可按 AppID 查询 Steam 商店的公开信息，并通过 CLI 预览本地清单和发起单个 Depot 的匿名下载。清单文件和目录路径保留在当前会话中。外部工具配置及验证结果见 [Linux Steam 清单与游戏库工具计划](linux-steam-library-plan.md)。
