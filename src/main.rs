use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, RichText, ScrollArea, TextEdit};
use egui_mineral_material::prelude::Runtime;
use egui_mineral_material::{Material, MaterialUi, Rt};
use egui_file_dialog::{FileDialog, FileDialogLabels};

mod covers;
mod depot;
mod design;
mod env_panel;
mod fonts;
mod journal;
mod jobs;
mod pages;
mod settings;
mod sls;
mod steam;
mod steam_library;
mod steam_store;
mod widgets;

use jobs::{RepairJob, RestartState, StoreLookup, StoreSearch, run_add_game};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Page {
    Library,
    Add,
    Environment,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Library => "游戏库",
            Self::Add => "添加游戏",
            Self::Environment => "环境",
            Self::Settings => "设置",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Library => "已发现的 Steam 库与安装记录",
            Self::Add => "把 AppID 写进 SLSsteam，让 Steam 原生下载",
            Self::Environment => "Steam、SLSsteam 与注入状态",
            Self::Settings => "启动行为与路径",
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("io.github.ACG-j.GameHaven")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([design::WINDOW_MIN_WIDTH, design::WINDOW_MIN_HEIGHT]),
        ..Default::default()
    };

    eframe::run_native(
        "游栖 · GameHaven",
        options,
        Box::new(|creation| {
            let app = GameHaven::new(&creation.egui_ctx)?;
            Ok(Box::new(app))
        }),
    )
}

pub(crate) struct GameHaven {
    pub(crate) rt: Arc<Rt>,
    pub(crate) page: Page,
    pub(crate) app_id: String,
    pub(crate) search_term: String,
    pub(crate) store_lookup: StoreLookup,
    pub(crate) search: StoreSearch,
    pub(crate) manifest: Option<PathBuf>,
    pub(crate) library: Option<PathBuf>,
    pub(crate) depot_manifest: Option<PathBuf>,
    pub(crate) depot_input: String,
    pub(crate) manifest_input: String,
    pub(crate) manifest_download: Option<Receiver<Result<PathBuf, String>>>,
    pub(crate) manifest_dialog: FileDialog,
    pub(crate) library_dialog: FileDialog,
    pub(crate) depot_manifest_dialog: FileDialog,
    pub(crate) depot_manifest_result: Option<Result<String, String>>,
    pub(crate) download: DepotPanel,
    pub(crate) libraries: Vec<steam_library::Library>,
    pub(crate) installed: Option<(u32, Option<PathBuf>)>,
    pub(crate) install_request: Option<(u32, Result<(), String>)>,
    pub(crate) covers: covers::Covers,
    pub(crate) env: env_panel::EnvPanel,
    pub(crate) restart: RestartState,
    pub(crate) add_result: Option<Result<String, String>>,
    pub(crate) remove_result: Option<Result<String, String>>,
    pub(crate) account_result: Option<Result<String, String>>,
    pub(crate) launch_result: Option<Result<String, String>>,
    pub(crate) undo_result: Option<Result<String, String>>,
    pub(crate) repair: RepairJob,
    pub(crate) add_game: Option<Receiver<Result<String, String>>>,
    pub(crate) auto_restart: bool,
    saved_auto_restart: bool,
}

impl GameHaven {
    fn new(
        ctx: &egui::Context,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let rt = Runtime::new(ctx, Material::default())
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?;
        fonts::configure(ctx);
        let labels = FileDialogLabels {
            title_select_file: "选择 Steam 清单".into(),
            title_select_directory: "选择目标库目录".into(),
            cancel: "取消".into(),
            cancel_button: "取消".into(),
            open_button: "选择".into(),
            heading_places: "位置".into(),
            heading_pinned: "收藏".into(),
            heading_devices: "设备".into(),
            heading_removable_devices: "可移动设备".into(),
            home_dir: "主目录".into(),
            desktop_dir: "桌面".into(),
            documents_dir: "文档".into(),
            downloads_dir: "下载".into(),
            audio_dir: "音乐".into(),
            pictures_dir: "图片".into(),
            videos_dir: "视频".into(),
            selected_file: "已选文件".into(),
            selected_directory: "已选目录".into(),
            file_name: "文件名".into(),
            file_filter_all_files: "所有文件".into(),
            reload: "刷新".into(),
            working_directory: "工作目录".into(),
            show_hidden: "显示隐藏项".into(),
            show_system_files: "显示系统文件".into(),
            pin_folder: "收藏".into(),
            unpin_folder: "取消收藏".into(),
            rename_pinned_folder: "重命名收藏".into(),
            ..Default::default()
        };
        let picker = |id, labels| {
            FileDialog::new()
                .id(egui::Id::unique(id))
                .labels(labels)
                .as_modal(true)
                .default_size([440.0, 270.0])
                .show_new_folder_button(false)
        };

        Ok(Self {
            rt,
            page: Page::Library,
            app_id: String::new(),
            search_term: String::new(),
            store_lookup: StoreLookup::default(),
            search: StoreSearch::default(),
            manifest: None,
            library: None,
            depot_manifest: None,
            depot_input: String::new(),
            manifest_input: String::new(),
            manifest_download: None,
            manifest_dialog: picker("manifest", labels.clone()),
            library_dialog: picker("library", labels.clone()),
            depot_manifest_dialog: picker("depot-manifest", labels),
            depot_manifest_result: None,
            download: DepotPanel::default(),
            libraries: steam_library::discover(),
            installed: None,
            install_request: None,
            covers: covers::Covers::default(),
            env: env_panel::EnvPanel::new(ctx),
            restart: RestartState::Idle,
            add_result: None,
            remove_result: None,
            account_result: None,
            launch_result: None,
            undo_result: None,
            repair: RepairJob::Idle,
            add_game: None,
            auto_restart: settings::load().auto_restart,
            saved_auto_restart: settings::load().auto_restart,
        })
    }

    fn poll_background(&mut self, ctx: &egui::Context) {
        self.store_lookup.poll();
        self.search.poll();
        self.covers.poll(ctx);
        self.env.poll(ctx);
        self.poll_restart(ctx);
        self.poll_repair(ctx);
        self.poll_manifest_download(ctx);
        self.poll_add_game(ctx);
        self.download.poll(ctx, &mut self.app_id);
    }

    fn poll_restart(&mut self, ctx: &egui::Context) {
        let result = match &self.restart {
            RestartState::Running(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                    None
                }
                Err(TryRecvError::Disconnected) => {
                    Some(Err("重启任务意外结束。".to_owned()))
                }
            },
            _ => None,
        };
        if let Some(result) = result {
            self.restart = RestartState::Done(result);
            self.env.refresh(ctx);
        }
    }

    pub(crate) fn start_restart(&mut self, ctx: &egui::Context) {
        if matches!(self.restart, RestartState::Running(_)) {
            return;
        }
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("steam-restart".into())
            .spawn(move || {
                let result = steam::process::restart();
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => self.restart = RestartState::Running(receiver),
            Err(error) => {
                self.restart = RestartState::Done(Err(format!("无法启动任务（{error}）。")));
            }
        }
    }

    pub(crate) fn add_to_slssteam(&mut self, app_id: u32, ctx: &egui::Context) {
        let Some(path) = steam::env::sls_config_path() else {
            self.add_result = Some(Err("没有找到 SLSsteam 的 config.yaml。".to_owned()));
            return;
        };
        journal::backup("添加游戏", &[path.as_path()]);
        let config = sls::config::Config::new(path);
        let dlcs = self.store_lookup.dlcs(app_id);
        let mut added = 0usize;
        let mut errors = Vec::new();

        match config.add_app(app_id) {
            Ok(true) => added += 1,
            Ok(false) => {}
            Err(error) => errors.push(error),
        }
        for dlc in &dlcs {
            match config.add_app(*dlc) {
                Ok(true) => added += 1,
                Ok(false) => {}
                Err(error) => errors.push(error),
            }
            if let Err(error) = config.add_dlc(app_id, *dlc, &format!("DLC {dlc}")) {
                errors.push(error);
            }
        }

        if !errors.is_empty() {
            self.add_result = Some(Err(errors.join(" ")));
            return;
        }
        let message = if dlcs.is_empty() {
            format!("已把 {app_id} 加入 AdditionalApps（新增 {added} 项）。")
        } else {
            format!(
                "已把 {app_id} 与 {} 个 DLC 加入 AdditionalApps（新增 {added} 项）。",
                dlcs.len()
            )
        };
        journal::record(&format!(
            "写入 AdditionalApps：{app_id}（DLC {} 个）",
            dlcs.len()
        ));
        self.add_result = Some(Ok(message));
        if self.auto_restart {
            self.start_restart(ctx);
        }
    }

    pub(crate) fn remove_from_slssteam(&mut self, app_id: u32, ctx: &egui::Context) {
        let Some(path) = steam::env::sls_config_path() else {
            self.remove_result = Some(Err("没有找到 SLSsteam 的 config.yaml。".to_owned()));
            return;
        };
        journal::backup("移除注入", &[path.as_path()]);
        let config = sls::config::Config::new(path);
        let mut errors = Vec::new();
        if let Err(error) = config.remove_app(app_id) {
            errors.push(error);
        }
        if let Err(error) = config.remove_dlc(app_id) {
            errors.push(error);
        }
        if errors.is_empty() {
            journal::record(&format!("移除注入 AppID {app_id}"));
            self.remove_result = Some(Ok(format!(
                "已从 SLSsteam 配置移除 {app_id}，重启 Steam 后生效。"
            )));
            self.env.refresh(ctx);
        } else {
            self.remove_result = Some(Err(errors.join(" ")));
        }
    }

    pub(crate) fn start_repair(
        &mut self,
        library: &steam_library::Library,
        app_id: u32,
        installdir: &str,
        ctx: &egui::Context,
    ) {
        if matches!(self.repair, RepairJob::Running(_)) {
            return;
        }
        let steamapps = library.path.join("steamapps");
        let game_dir = steamapps.join("common").join(installdir);
        let acf = steamapps.join(format!("appmanifest_{app_id}.acf"));
        journal::backup("修复 ACF", &[acf.as_path()]);
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("steam-acf".into())
            .spawn(move || {
                let size = steam::acf::dir_size(&game_dir);
                let result = steam::acf::repair(&steamapps, app_id, size).map(|path| {
                    journal::record(&format!("修复 ACF：{app_id}，SizeOnDisk {size}"));
                    format!("已修复 {}（{} 字节）。", path.display(), size)
                });
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => self.repair = RepairJob::Running(receiver),
            Err(error) => {
                self.repair = RepairJob::Done(Err(format!("无法启动修复任务（{error}）。")));
            }
        }
    }

    fn poll_repair(&mut self, ctx: &egui::Context) {
        let result = match &self.repair {
            RepairJob::Running(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                    None
                }
                Err(TryRecvError::Disconnected) => Some(Err("修复任务意外结束。".to_owned())),
            },
            _ => None,
        };
        if let Some(result) = result {
            self.repair = RepairJob::Done(result);
        }
    }

    pub(crate) fn launch_game(&mut self, library: &steam_library::Library, app_id: u32) {
        self.launch_result = Some(
            library
                .client
                .run(app_id)
                .map(|()| format!("已请求启动 {app_id}。")),
        );
    }

    pub(crate) fn switch_account(&mut self, account: String, _ctx: &egui::Context) {
        let Some(root) = self.libraries.first().map(|library| library.path.clone()) else {
            self.account_result = Some(Err("没有找到 Steam 库。".to_owned()));
            return;
        };
        let loginusers = root.join("config/loginusers.vdf");
        let config = root.join("config/config.vdf");
        journal::backup("切换账号", &[loginusers.as_path(), config.as_path()]);
        match steam::accounts::switch(&root, &account) {
            Ok(()) => {
                journal::record(&format!("切换自动登录账号为 {account}"));
                self.account_result = Some(Ok(format!(
                    "已切换到 {account}，重启 Steam 后生效。"
                )));
            }
            Err(error) => self.account_result = Some(Err(error)),
        }
    }

    /// 一键添加：查 appinfo → 写 AdditionalApps/DlcData → 拉 depot manifest → 可选重启。
    pub(crate) fn undo_last_write(&mut self, ctx: &egui::Context) {
        match journal::undo_last() {
            Ok(message) => {
                self.undo_result = Some(Ok(message));
                self.env.refresh(ctx);
                self.libraries = steam_library::discover();
                self.installed = None;
            }
            Err(error) => self.undo_result = Some(Err(error)),
        }
    }

    pub(crate) fn start_add_game(&mut self, app_id: u32, ctx: &egui::Context) {
        if self.add_game.is_some() {
            return;
        }
        let Some(steam_root) = steam::env::steam_root() else {
            self.add_result = Some(Err("没有找到 Steam 安装目录。".to_owned()));
            return;
        };
        let Some(library) = self
            .library
            .clone()
            .or_else(|| self.libraries.first().map(|library| library.path.clone()))
        else {
            self.add_result = Some(Err("没有找到 Steam 库。".to_owned()));
            return;
        };
        let Some(config_path) = steam::env::sls_config_path() else {
            self.add_result = Some(Err("没有找到 SLSsteam 的 config.yaml。".to_owned()));
            return;
        };
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("game-add".into())
            .spawn(move || {
                let result = run_add_game(app_id, &steam_root, &library, &config_path);
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.add_result = None;
                self.add_game = Some(receiver);
            }
            Err(error) => {
                self.add_result = Some(Err(format!("无法启动添加任务（{error}）。")));
            }
        }
    }

    fn poll_add_game(&mut self, ctx: &egui::Context) {
        let result = match &self.add_game {
            Some(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                    None
                }
                Err(TryRecvError::Disconnected) => Some(Err("添加任务意外结束。".to_owned())),
            },
            None => None,
        };
        if let Some(result) = result {
            self.add_game = None;
            let added = result.is_ok();
            self.add_result = Some(result);
            if added && self.auto_restart {
                self.start_restart(ctx);
            }
        }
    }

    /// 从公开镜像下载 manifest，成功后走与手动导入相同的落盘流程。
    pub(crate) fn start_manifest_download(&mut self, ctx: &egui::Context) {
        if self.manifest_download.is_some() {
            return;
        }
        let Ok(depot_id) = self.depot_input.trim().parse::<u32>() else {
            self.depot_manifest_result = Some(Err("Depot ID 必须是数字。".to_owned()));
            return;
        };
        let Ok(manifest_id) = self.manifest_input.trim().parse::<u64>() else {
            self.depot_manifest_result = Some(Err("Manifest ID 必须是数字。".to_owned()));
            return;
        };
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("manifest-download".into())
            .spawn(move || {
                let result = steam::manifest::download(depot_id, manifest_id).and_then(|bytes| {
                    steam::manifest::cache_bytes(depot_id, manifest_id, &bytes)
                        .map(|file| file.path)
                });
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => self.manifest_download = Some(receiver),
            Err(error) => {
                self.depot_manifest_result =
                    Some(Err(format!("无法启动下载任务（{error}）。")));
            }
        }
    }

    fn poll_manifest_download(&mut self, ctx: &egui::Context) {
        let result = match &self.manifest_download {
            Some(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                    None
                }
                Err(TryRecvError::Disconnected) => {
                    Some(Err("manifest 下载任务意外结束。".to_owned()))
                }
            },
            None => None,
        };
        if let Some(result) = result {
            self.manifest_download = None;
            match result {
                Ok(path) => {
                    self.depot_manifest = Some(path);
                    self.import_depot_manifest(ctx);
                }
                Err(error) => self.depot_manifest_result = Some(Err(error)),
            }
        }
    }

    /// 把用户提供的 `.manifest` 放到 depotcache，写 ManifestIds，并为当前 AppID 生成安装记录。
    pub(crate) fn import_depot_manifest(&mut self, _ctx: &egui::Context) {
        let Some(path) = self.depot_manifest.clone() else {
            self.depot_manifest_result = Some(Err("先选择一个 .manifest 文件。".to_owned()));
            return;
        };
        let Some(file) = steam::manifest::parse(&path) else {
            self.depot_manifest_result = Some(Err(
                "文件名不是 <depot>_<manifest>.manifest，无法解析。".to_owned(),
            ));
            return;
        };
        let Some(steam_root) = steam::env::steam_root() else {
            self.depot_manifest_result = Some(Err("没有找到 Steam 安装目录。".to_owned()));
            return;
        };
        let Some(library) = self
            .library
            .clone()
            .or_else(|| self.libraries.first().map(|library| library.path.clone()))
        else {
            self.depot_manifest_result = Some(Err("没有找到 Steam 库。".to_owned()));
            return;
        };

        match steam::manifest::install(&steam_root, &library, &file) {
            Ok(written) => {
                let mut backups: Vec<PathBuf> = vec![steam_root.join("depotcache")];
                if let Some(config_path) = steam::env::sls_config_path() {
                    backups.clear();
                    backups.push(config_path);
                }
                if let Ok(app_id) = design::validate_app_id(&self.app_id) {
                    backups.push(library.join("steamapps").join(format!("appmanifest_{app_id}.acf")));
                }
                let refs: Vec<&std::path::Path> = backups.iter().map(PathBuf::as_path).collect();
                journal::backup("导入 manifest", &refs);
                let mut notes = vec![format!("manifest 已写入 {} 处", written.len())];
                if let Some(config_path) = steam::env::sls_config_path() {
                    let config = sls::config::Config::new(config_path);
                    match config.set_manifest(file.depot_id, file.manifest_id) {
                        Ok(true) => notes.push("ManifestIds 已更新".to_owned()),
                        Ok(false) => notes.push("ManifestIds 已是该版本".to_owned()),
                        Err(error) => notes.push(format!("ManifestIds 写入失败：{error}")),
                    }
                }
                if let Ok(app_id) = design::validate_app_id(&self.app_id) {
                    let name = self
                        .store_lookup
                        .name(app_id)
                        .unwrap_or_else(|| format!("App {app_id}"));
                    let installdir = steam::acf::sanitize_installdir(&name);
                    let game = steam::acf::GameInfo {
                        app_id,
                        name: &name,
                        installdir: &installdir,
                        build_id: "0",
                        size_on_disk: 0,
                        depots: &[(file.depot_id, file.manifest_id)],
                    };
                    match steam::acf::write(&library.join("steamapps"), &game) {
                        Ok(acf) => notes.push(format!("已生成 {}", acf.display())),
                        Err(error) => notes.push(format!("ACF 生成失败：{error}")),
                    }
                } else {
                    notes.push("AppID 为空，未生成 ACF".to_owned());
                }
                journal::record(&format!(
                    "导入 manifest：depot {} / manifest {}",
                    file.depot_id, file.manifest_id
                ));
                self.depot_manifest_result = Some(Ok(notes.join("；")));
            }
            Err(error) => self.depot_manifest_result = Some(Err(error)),
        }
    }

    fn sidebar(&mut self, ui: &mut MaterialUi<'_>) {
        let weak = design::on_surface_variant(ui);
        ui.add_space(design::SPACE_5);
        ui.indent("sidebar-brand", |ui| {
            ui.label(RichText::new("游栖").size(design::TEXT_HEADLINE).strong());
            ui.label(
                RichText::new("GameHaven")
                    .size(design::TEXT_LABEL)
                    .color(weak),
            );
        });
        ui.add_space(design::SPACE_5);

        for page in [Page::Library, Page::Add, Page::Environment, Page::Settings] {
            if widgets::nav_item(ui, page.title(), self.page == page) {
                self.page = page;
            }
            ui.add_space(design::SPACE_1);
        }

        let remaining = ui.available_height();
        if remaining > 120.0 {
            ui.add_space(remaining - 112.0);
        }
        ui.add_space(design::SPACE_3);
        ui.separator();
        ui.add_space(design::SPACE_3);
        ui.indent("sidebar-status", |ui| {
            let weak = design::on_surface_variant(ui);
            let summary = self.env.summary();
            match summary {
                Some((ok, warn, error)) => {
                    let text = if error > 0 {
                        format!("体检 {ok} 正常 · {error} 异常")
                    } else if warn > 0 {
                        format!("体检 {ok} 正常 · {warn} 注意")
                    } else {
                        format!("体检 {ok} 项全正常")
                    };
                    ui.label(RichText::new(text).size(design::TEXT_LABEL).color(weak));
                }
                None => {
                    ui.label(
                        RichText::new("正在检查…")
                            .size(design::TEXT_LABEL)
                            .color(weak),
                    );
                }
            }
            ui.add_space(design::SPACE_1);
            ui.horizontal(|ui| {
                let running = self.env.steam_running();
                let color = if running {
                    design::OK
                } else {
                    weak
                };
                ui.label(RichText::new("●").size(design::TEXT_LABEL).color(color));
                ui.label(
                    RichText::new(if running {
                        "Steam 运行中"
                    } else {
                        "Steam 未运行"
                    })
                    .size(design::TEXT_LABEL),
                );
            });
        });
    }

    pub(crate) fn show_steam_install(&mut self, ui: &mut MaterialUi<'_>, app_id: u32) {
        ui.add_space(14.0);
        ui.separator();
        ui.heading("Steam 客户端安装");
        if self
            .installed
            .as_ref()
            .is_none_or(|(id, _)| *id != app_id)
        {
            let path = self
                .libraries
                .iter()
                .find_map(|library| steam_library::installed_location(library, app_id));
            self.installed = Some((app_id, path));
        }
        if let Some((_, Some(path))) = &self.installed {
            ui.label(format!("已安装 · {}", path.display()));
            return;
        }
        let mut has_client = false;
        for client in [
            steam_library::Client::Native,
            steam_library::Client::Flatpak,
        ] {
            if self
                .libraries
                .iter()
                .any(|library| library.client == client)
            {
                has_client = true;
                if ui.button(format!("在 {} 中安装", client.name())).clicked() {
                    self.install_request = Some((app_id, client.install(app_id)));
                }
            }
        }
        if !has_client {
            ui.weak("没有发现 Steam 库。请先启动 Steam 并完成初始化。");
        }
        if let Some((id, result)) = &self.install_request
            && *id == app_id
        {
            match result {
                Ok(()) => {
                    ui.label("已发起安装请求。请在 Steam 中确认许可证和安装目录。");
                }
                Err(error) => {
                    let error_color = ui.visuals().error_fg_color;
                    ui.colored_label(error_color, error);
                }
            }
        }
    }
}

impl eframe::App for GameHaven {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_background(ui.ctx());

        let rt = self.rt.clone();
        rt.pass(ui, |ui| self.shell(ui));

        self.manifest_dialog.update(ui.ctx());
        if let Some(path) = self.manifest_dialog.take_picked() {
            self.manifest = Some(path);
            self.download = DepotPanel::default();
            ui.ctx().request_repaint();
        }
        self.library_dialog.update(ui.ctx());
        if let Some(path) = self.library_dialog.take_picked() {
            self.library = Some(path);
            ui.ctx().request_repaint();
        }
        self.depot_manifest_dialog.update(ui.ctx());
        if let Some(path) = self.depot_manifest_dialog.take_picked() {
            self.depot_manifest = Some(path);
            self.depot_manifest_result = None;
            ui.ctx().request_repaint();
        }

        // 开关变化时落盘，下次启动沿用。
        if self.auto_restart != self.saved_auto_restart {
            settings::save(&settings::Settings {
                auto_restart: self.auto_restart,
            });
            self.saved_auto_restart = self.auto_restart;
        }
    }
}

impl GameHaven {
    fn shell(&mut self, ui: &mut MaterialUi<'_>) {
        let rt = self.rt.clone();
        let height = ui.available_height();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(design::SIDEBAR_WIDTH, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.sidebar(ui),
            );
            ui.separator();
            let mut content = rt.ui(ui);
            content.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                self.top_bar(ui);
                ui.separator();
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut mui = rt.ui(ui);
                        mui.add_space(design::SPACE_5);
                        let available = mui.available_width();
                        let width = (available - design::SPACE_5 * 2.0)
                            .min(design::CONTENT_MAX_WIDTH)
                            .max(320.0);
                        mui.horizontal_top(|ui| {
                            ui.add_space(design::SPACE_5);
                            ui.allocate_ui_with_layout(
                                egui::vec2(width, ui.available_height()),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| match self.page {
                                    Page::Library => self.library_page(ui),
                                    Page::Add => self.add_page(ui),
                                    Page::Environment => self.environment_page(ui),
                                    Page::Settings => self.settings_page(ui),
                                },
                            );
                        });
                    });
            });
        });
    }

    fn top_bar(&mut self, ui: &mut MaterialUi<'_>) {
        let variant = design::on_surface_variant(ui);
        ui.add_space(design::SPACE_3);
        ui.horizontal(|ui| {
            ui.add_space(design::SPACE_5);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.page.title())
                        .size(design::TEXT_HEADLINE)
                        .strong(),
                );
                ui.label(
                    RichText::new(self.page.subtitle())
                        .size(design::TEXT_BODY)
                        .color(variant),
                );
            });
        });
        ui.add_space(design::SPACE_3);
    }
}

#[derive(Default)]
pub(crate) struct DepotPanel {
    job: Option<depot::Job>,
    plan: Option<depot::Plan>,
    selected: usize,
    result: Option<Result<&'static str, String>>,
    last_progress: depot::Progress,
    downloaded: Option<(u32, PathBuf)>,
    import_library: Option<steam_library::Library>,
    import_folder: String,
    staged: Option<(steam_library::Library, u32)>,
    handoff: Option<Result<(), String>>,
}

impl DepotPanel {
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn start(&mut self, request: depot::Request) {
        self.result = None;
        self.last_progress = depot::Progress::default();
        match depot::Job::start(request) {
            Ok(job) => self.job = Some(job),
            Err(err) => self.result = Some(Err(format!("无法启动任务（{err}）。"))),
        }
    }

    fn poll(&mut self, ctx: &egui::Context, app_id: &mut String) {
        let Some(job) = &mut self.job else {
            return;
        };
        let Some(result) = job.try_finish() else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        self.last_progress = job.progress();
        self.job = None;
        match result {
            Ok(depot::Outcome::Inspected(plan)) => {
                if app_id.trim().is_empty() {
                    *app_id = plan.app_id.to_string();
                }
                self.plan = Some(plan);
                self.selected = 0;
            }
            Ok(depot::Outcome::Downloaded(id, output)) => {
                self.downloaded = Some((id, output));
                self.result = Some(Ok("Depot 下载完成。"));
            }
            Ok(depot::Outcome::Staged(library, id)) => {
                self.staged = Some((library, id));
                self.result = Some(Ok("已有文件已放入 Steam 库。"));
            }
            Ok(depot::Outcome::Cancelled) => self.result = Some(Ok("任务已取消。")),
            Err(message) => self.result = Some(Err(message)),
        }
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut MaterialUi<'_>,
        manifest: &Option<PathBuf>,
        library: &Option<PathBuf>,
        app_id: Option<u32>,
    ) {
        let Some(manifest) = manifest else {
            return;
        };
        ui.add_space(12.0);
        if let Some(job) = &self.job {
            let progress = job.progress();
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if job.cancelling() {
                    "正在取消…"
                } else if self.downloaded.is_some() {
                    "正在复制到 Steam 库…"
                } else if self.plan.is_some() {
                    "正在下载 Depot…"
                } else {
                    "正在解析清单…"
                });
                if ui
                    .add_enabled(!job.cancelling(), egui::Button::new("取消任务"))
                    .clicked()
                {
                    job.cancel();
                }
            });
            if let Some(fraction) = progress.fraction {
                ui.add(egui::ProgressBar::new(fraction).show_percentage());
            }
            Self::show_progress(ui, &progress);
            return;
        }

        if let Some(plan) = &self.plan {
            ui.label(format!(
                "清单 AppID {} · {} 个 Depot",
                plan.app_id,
                plan.depots.len()
            ));
            let depot_label = |depot: &depot::Depot| match depot.manifest {
                Some(id) => format!("Depot {} · Manifest {id}", depot.id),
                None => format!("Depot {} · 当前公开版本", depot.id),
            };
            egui::ComboBox::from_id_salt("manifest-depots")
                .selected_text(depot_label(&plan.depots[self.selected]))
                .show_ui(ui, |ui| {
                    for (index, depot) in plan.depots.iter().enumerate() {
                        ui.selectable_value(&mut self.selected, index, depot_label(depot));
                    }
                });
            if app_id != Some(plan.app_id) {
                ui.weak("请将输入的 AppID 改为清单中的 AppID。");
            }
            if ui
                .add_enabled(
                    library.is_some() && app_id == Some(plan.app_id),
                    egui::Button::new("匿名下载所选 Depot"),
                )
                .clicked()
                && let Some(library) = library
            {
                self.downloaded = None;
                self.staged = None;
                self.handoff = None;
                self.start(depot::Request::Fetch {
                    app_id: plan.app_id,
                    depot: plan.depots[self.selected],
                    library: library.clone(),
                });
            }
        } else if ui.button("解析清单").clicked() {
            self.start(depot::Request::Inspect(manifest.clone()));
        }

        match &self.result {
            Some(Ok(message)) => {
                ui.label(*message);
            }
            Some(Err(message)) => {
                let error_color = ui.visuals().error_fg_color;
                ui.colored_label(error_color, message);
            }
            None => {}
        }
        Self::show_progress(ui, &self.last_progress);
    }

    pub(crate) fn show_import(&mut self, ui: &mut MaterialUi<'_>, libraries: &[steam_library::Library]) {
        let Some((id, source)) = &self.downloaded else {
            return;
        };
        if self.job.is_some() {
            return;
        }
        ui.separator();
        ui.heading("复用已下载文件");
        if let Some((library, id)) = &self.staged {
            let verifying = steam_library::installed_location(library, *id).is_some();
            let button = if verifying {
                "在 Steam 中校验"
            } else {
                "在 Steam 中继续安装"
            };
            if ui.button(button).clicked() {
                self.handoff = Some(if verifying {
                    library.client.verify(*id)
                } else {
                    library.client.install(*id)
                });
            }
            match &self.handoff {
                Some(Ok(())) => {
                    ui.label("已向 Steam 发出请求。请在同一 Steam 库继续安装或校验。");
                }
                Some(Err(error)) => {
                    let error_color = ui.visuals().error_fg_color;
                    ui.colored_label(error_color, error);
                }
                None => {}
            }
            return;
        }
        if libraries.is_empty() {
            ui.label("未发现 Steam 库。请先在 Steam 中配置库目录，再刷新 Steam 库。");
            return;
        }
        if !self
            .import_library
            .as_ref()
            .is_some_and(|library| libraries.contains(library))
        {
            self.import_library = libraries
                .iter()
                .find(|library| steam_library::app_record(library, *id).is_some())
                .or_else(|| {
                    libraries
                        .iter()
                        .find(|library| source.parent() == Some(library.path.as_path()))
                })
                .or_else(|| libraries.first())
                .cloned();
        }
        let library_label = |library: &steam_library::Library| {
            format!("{} · {}", library.client.name(), library.path.display())
        };
        egui::ComboBox::from_id_salt("import-library")
            .width(290.0)
            .selected_text(
                self.import_library
                    .as_ref()
                    .map(library_label)
                    .unwrap_or_default(),
            )
            .show_ui(ui, |ui| {
                for library in libraries {
                    if ui
                        .selectable_value(
                            &mut self.import_library,
                            Some(library.clone()),
                            library_label(library),
                        )
                        .clicked()
                    {
                        self.import_folder.clear();
                    }
                }
            });
        let Some(library) = &self.import_library else {
            return;
        };
        let registered = steam_library::app_record(library, *id);
        let folder = if let Some((folder, _)) = &registered {
            ui.label(format!("Steam 安装目录 · {folder}"));
            folder.as_str()
        } else {
            let label = ui.label("Steam 安装目录名（steamapps/common 下）");
            ui.add(TextEdit::singleline(&mut self.import_folder).desired_width(260.0))
                .labelled_by(label.id);
            ui.weak("可先在 Steam 开始安装、暂停，再刷新库以自动读取目录名。");
            self.import_folder.trim()
        };
        if ui
            .add_enabled(
                steam_library::valid_folder(folder),
                egui::Button::new("复制到 Steam 库"),
            )
            .clicked()
        {
            self.start(depot::Request::Stage {
                app_id: *id,
                source: source.clone(),
                library: library.clone(),
                folder: folder.to_owned(),
            });
        }
    }

    fn show_progress(ui: &mut egui::Ui, progress: &depot::Progress) {
        if let Some(path) = &progress.output {
            ui.label(format!("输出目录 · {}", path.display()));
        }
        if !progress.detail.is_empty() {
            ui.weak(&progress.detail);
        }
    }
}
