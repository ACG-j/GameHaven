//! 后台任务状态：商店查询、搜索、修复、重启与一键添加。

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use eframe::egui::{self, RichText};

use crate::journal;
use crate::sls;
use crate::steam;
use crate::steam_store;

pub(crate) type LookupResult = Result<Option<steam_store::StoreApp>, String>;

pub(crate) enum RepairJob {
    Idle,
    Running(Receiver<Result<String, String>>),
    Done(Result<String, String>),
}

pub(crate) enum RestartState {
    Idle,
    Running(Receiver<Result<String, String>>),
    Done(Result<String, String>),
}

#[derive(Default)]
pub(crate) enum StoreLookup {
    #[default]
    Idle,
    Pending(u32, Receiver<LookupResult>),
    Finished(u32, LookupResult),
}

impl StoreLookup {
    pub(crate) fn busy(&self) -> bool {
        matches!(self, Self::Pending(..))
    }

    /// 查询结果里的游戏名；未查询或不匹配时为 `None`。
    pub(crate) fn name(&self, app_id: u32) -> Option<String> {
        match self {
            Self::Finished(id, Ok(Some(app))) if *id == app_id => Some(app.name.clone()),
            _ => None,
        }
    }

    /// 查询结果里的 DLC AppID；未查询或不匹配时为空。
    pub(crate) fn dlcs(&self, app_id: u32) -> Vec<u32> {
        match self {
            Self::Finished(id, Ok(Some(app))) if *id == app_id => app.dlc.clone(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn start(&mut self, app_id: u32, ctx: &egui::Context) {
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        *self = match thread::Builder::new()
            .name("steam-store".into())
            .spawn(move || {
                let _ = sender.send(steam_store::lookup(app_id));
                ctx.request_repaint();
            }) {
            Ok(_) => Self::Pending(app_id, receiver),
            Err(_) => Self::Finished(app_id, Err("无法启动 Steam 商店查询，请重试。".into())),
        };
    }

    pub(crate) fn poll(&mut self) {
        let Self::Pending(app_id, receiver) = self else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("Steam 商店查询意外结束，请重试。".into()),
        };
        *self = Self::Finished(*app_id, result);
    }

    pub(crate) fn show(&self, ui: &mut egui::Ui, current_id: Option<u32>) {
        match self {
            Self::Pending(app_id, _) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("正在查询 AppID {app_id}…"));
                });
            }
            Self::Finished(app_id, result) if Some(*app_id) == current_id => match result {
                Ok(Some(app)) => {
                    ui.label(RichText::new(&app.name).strong());
                    let platforms: Vec<_> = app
                        .platforms
                        .iter()
                        .flat_map(|platforms| {
                            [
                                (platforms.windows, "Windows"),
                                (platforms.mac, "macOS"),
                                (platforms.linux, "Linux"),
                            ]
                            .into_iter()
                            .filter_map(|(supported, name)| supported.then_some(name))
                        })
                        .collect();
                    if platforms.is_empty() {
                        ui.weak("商店未列出支持平台。");
                    } else {
                        ui.label(format!("商店列出的平台 · {}", platforms.join(" / ")));
                    }
                }
                Ok(None) => {
                    ui.label("Steam 商店未提供这个 AppID 的公开信息。");
                }
                Err(message) => {
                    ui.colored_label(ui.visuals().error_fg_color, message);
                }
            },
            _ => {}
        }
    }
}

#[derive(Default)]
pub(crate) enum StoreSearch {
    #[default]
    Idle,
    Pending(Receiver<Result<Vec<steam_store::SearchItem>, String>>),
    Finished(Result<Vec<steam_store::SearchItem>, String>),
}

impl StoreSearch {
    pub(crate) fn busy(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    pub(crate) fn start(&mut self, term: String, ctx: &egui::Context) {
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        *self = match thread::Builder::new()
            .name("steam-search".into())
            .spawn(move || {
                let _ = sender.send(steam_store::search(&term));
                ctx.request_repaint();
            }) {
            Ok(_) => Self::Pending(receiver),
            Err(_) => Self::Finished(Err("无法启动商店搜索，请重试。".to_owned())),
        };
    }

    pub(crate) fn poll(&mut self) {
        let Self::Pending(receiver) = self else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => *self = Self::Finished(result),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                *self = Self::Finished(Err("商店搜索意外结束。".to_owned()));
            }
        }
    }

    pub(crate) fn result(&self) -> Option<&Result<Vec<steam_store::SearchItem>, String>> {
        match self {
            Self::Finished(result) => Some(result),
            _ => None,
        }
    }
}

pub(crate) fn run_add_game(
    app_id: u32,
    steam_root: &std::path::Path,
    library: &std::path::Path,
    config_path: &std::path::Path,
) -> Result<String, String> {
    let info = steam::appinfo::fetch(app_id)?;
    journal::backup("一键添加", &[config_path]);
    let config = sls::config::Config::new(config_path.to_owned());
    config.add_app(app_id)?;
    for dlc in &info.dlcs {
        let _ = config.add_app(*dlc);
        let _ = config.add_dlc(app_id, *dlc, &format!("DLC {dlc}"));
    }

    // 最多处理 6 个 depot：镜像覆盖有限，失败就交给 Steam 自己从 CDN 拉。
    let mut downloaded = 0usize;
    for (depot, manifest) in info.depots.iter().take(6) {
        if let Ok(bytes) = steam::manifest::download(*depot, *manifest)
            && let Ok(file) = steam::manifest::cache_bytes(*depot, *manifest, &bytes)
            && steam::manifest::install(steam_root, library, &file).is_ok()
        {
            let _ = config.set_manifest(*depot, *manifest);
            downloaded += 1;
        }
    }

    let name = if info.name.is_empty() {
        format!("App {app_id}")
    } else {
        info.name
    };
    journal::record(&format!(
        "一键添加 {app_id}（{name}），DLC {} 个，manifest {downloaded} 个",
        info.dlcs.len()
    ));
    Ok(format!(
        "已添加 {name}：DLC {} 个，manifest 就绪 {downloaded} 个。重启 Steam 后在库中安装。",
        info.dlcs.len()
    ))
}
