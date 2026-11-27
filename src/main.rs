use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, RichText, ScrollArea, TextEdit};
use egui_mineral_material::prelude::Runtime;
use egui_mineral_material::{Material, MaterialUi, Rt};
use egui_file_dialog::{FileDialog, FileDialogLabels};

mod depot;
mod env_panel;
mod fonts;
mod pages;
mod sls;
mod steam;
mod steam_library;
mod steam_store;
mod ui;

type LookupResult = Result<Option<steam_store::StoreApp>, String>;

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

    fn poll(&mut self) {
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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("io.github.ACG-j.GameHaven")
            .with_inner_size([1040.0, 720.0])
            .with_min_inner_size([880.0, 600.0]),
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
    pub(crate) store_lookup: StoreLookup,
    pub(crate) manifest: Option<PathBuf>,
    pub(crate) library: Option<PathBuf>,
    pub(crate) manifest_dialog: FileDialog,
    pub(crate) library_dialog: FileDialog,
    pub(crate) download: DepotPanel,
    pub(crate) libraries: Vec<steam_library::Library>,
    pub(crate) installed: Option<(u32, Option<PathBuf>)>,
    pub(crate) install_request: Option<(u32, Result<(), String>)>,
    pub(crate) env: env_panel::EnvPanel,
    pub(crate) restart: RestartState,
    pub(crate) add_result: Option<Result<String, String>>,
    pub(crate) auto_restart: bool,
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
            store_lookup: StoreLookup::default(),
            manifest: None,
            library: None,
            manifest_dialog: picker("manifest", labels.clone()),
            library_dialog: picker("library", labels),
            download: DepotPanel::default(),
            libraries: steam_library::discover(),
            installed: None,
            install_request: None,
            env: env_panel::EnvPanel::new(ctx),
            restart: RestartState::Idle,
            add_result: None,
            auto_restart: true,
        })
    }

    fn poll_background(&mut self, ctx: &egui::Context) {
        self.store_lookup.poll();
        self.env.poll(ctx);
        self.poll_restart(ctx);
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
        match sls::config::Config::new(path).add_app(app_id) {
            Ok(true) => {
                self.add_result = Some(Ok(format!("已把 {app_id} 加入 AdditionalApps。")));
                if self.auto_restart {
                    self.start_restart(ctx);
                }
            }
            Ok(false) => {
                self.add_result = Some(Ok(format!("{app_id} 已经在 AdditionalApps 里。")));
            }
            Err(error) => self.add_result = Some(Err(error)),
        }
    }

    fn sidebar(&mut self, ui: &mut MaterialUi<'_>) {
        let weak = ui.visuals().weak_text_color();
        ui.add_space(16.0);
        ui.indent("sidebar-top", |ui| {
            ui.label(RichText::new("游栖").size(19.0).strong());
            ui.label(
                RichText::new("GameHaven").size(11.0).color(weak),
            );
        });
        ui.add_space(20.0);

        for page in [
            Page::Library,
            Page::Add,
            Page::Environment,
            Page::Settings,
        ] {
            if ui::nav_item(ui, page.title(), self.page == page) {
                self.page = page;
            }
            ui.add_space(2.0);
        }

        let remaining = ui.available_height();
        if remaining > 96.0 {
            ui.add_space(remaining - 92.0);
        }
        ui.separator();
        ui.add_space(6.0);
        ui.indent("sidebar-bottom", |ui| {
            if let Some((ok, warn, error)) = self.env.summary() {
                let summary = if error > 0 {
                    format!("体检 {ok} 正常 · {error} 异常")
                } else if warn > 0 {
                    format!("体检 {ok} 正常 · {warn} 注意")
                } else {
                    format!("体检 {ok} 项全正常")
                };
                ui.label(RichText::new(summary).size(11.5).color(weak));
            } else {
                ui.label(RichText::new("正在检查…").size(11.5).color(weak));
            }
            ui.horizontal(|ui| {
                let running = self.env.steam_running();
                ui::status_dot(
                    ui,
                    if running {
                        ui::OK
                    } else {
                        ui.visuals().weak_text_color()
                    },
                );
                ui.label(
                    RichText::new(if running {
                        "Steam 运行中"
                    } else {
                        "Steam 未运行"
                    })
                    .size(12.0),
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
    }
}

impl GameHaven {
    fn shell(&mut self, ui: &mut MaterialUi<'_>) {
        let rt = self.rt.clone();
        let height = ui.available_height();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui::SIDEBAR_WIDTH, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.sidebar(ui),
            );
            ui.separator();
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut mui = rt.ui(ui);
                    mui.add_space(16.0);
                    mui.horizontal_top(|ui| {
                        ui.add_space(16.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(780.0, ui.available_height()),
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
