use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use eframe::egui::{self, CentralPanel, Frame, RichText, ScrollArea, TextEdit};
use egui_file_dialog::{FileDialog, FileDialogLabels};

mod depot;
mod fonts;
mod steam_library;
mod steam_store;

type LookupResult = Result<Option<steam_store::StoreApp>, String>;

#[derive(Default)]
enum StoreLookup {
    #[default]
    Idle,
    Pending(u32, Receiver<LookupResult>),
    Finished(u32, LookupResult),
}

impl StoreLookup {
    fn start(&mut self, app_id: u32, ctx: &egui::Context) {
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

    fn show(&self, ui: &mut egui::Ui, current_id: Option<u32>) {
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
            .with_inner_size([720.0, 480.0])
            .with_min_inner_size([480.0, 360.0]),
        ..Default::default()
    };

    eframe::run_native(
        "游栖 · GameHaven",
        options,
        Box::new(|creation| {
            fonts::configure(&creation.egui_ctx);
            Ok(Box::new(GameHaven::new()))
        }),
    )
}

struct GameHaven {
    app_id: String,
    store_lookup: StoreLookup,
    manifest: Option<PathBuf>,
    library: Option<PathBuf>,
    manifest_dialog: FileDialog,
    library_dialog: FileDialog,
    download: DepotPanel,
    libraries: Vec<steam_library::Library>,
    installed: Option<(u32, Option<PathBuf>)>,
    install_request: Option<(u32, Result<(), String>)>,
}

impl GameHaven {
    fn new() -> Self {
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

        Self {
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
        }
    }
}

impl eframe::App for GameHaven {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.store_lookup.poll();
        self.download.poll(ui.ctx(), &mut self.app_id);
        CentralPanel::default().show(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("游栖 · GameHaven").size(27.0).strong());
                ui.add_space(14.0);

                if ui.button("刷新 Steam 库").clicked() {
                    self.libraries = steam_library::discover();
                    self.installed = None;
                }
                ui.add_space(10.0);
                Frame::group(ui.style()).inner_margin(18).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.heading("准备 Steam 游戏");
                    ui.add_space(12.0);

                    let label = ui.label("Steam AppID");
                    let app_id = ui
                        .horizontal(|ui| {
                            ui.add(
                                TextEdit::singleline(&mut self.app_id)
                                    .hint_text("输入 Steam AppID")
                                    .desired_width(220.0),
                            )
                            .labelled_by(label.id);
                            let app_id = validate_app_id(&self.app_id);
                            let busy = matches!(self.store_lookup, StoreLookup::Pending(..));
                            if ui
                                .add_enabled(
                                    app_id.is_ok() && !busy,
                                    egui::Button::new("查询 Steam 商店"),
                                )
                                .clicked()
                                && let Ok(app_id) = app_id
                            {
                                self.store_lookup.start(app_id, ui.ctx());
                            }
                            app_id
                        })
                        .inner;
                    if !self.app_id.trim().is_empty()
                        && let Err(message) = app_id
                    {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }
                    self.store_lookup.show(ui, app_id.ok());

                    ui.add_space(12.0);
                    let had_manifest = self.manifest.is_some();
                    let browse_manifest = ui
                        .add_enabled_ui(self.download.job.is_none(), |ui| {
                            path_input(ui, "本地清单", "选择文件…", &mut self.manifest)
                        })
                        .inner;
                    if browse_manifest {
                        self.manifest_dialog.pick_file();
                    }
                    if had_manifest && self.manifest.is_none() {
                        self.download = DepotPanel::default();
                    }
                    ui.add_space(10.0);
                    let browse_library = ui
                        .add_enabled_ui(self.download.job.is_none(), |ui| {
                            let browse = path_input(ui, "目标库", "选择目录…", &mut self.library);
                            if !self.libraries.is_empty() {
                                ui.menu_button("已发现的 Steam 库", |ui| {
                                    for library in &self.libraries {
                                        let label = format!(
                                            "{} · {}",
                                            library.client.name(),
                                            library.path.display()
                                        );
                                        if ui.button(label).clicked() {
                                            self.library = Some(library.path.clone());
                                            ui.close();
                                        }
                                    }
                                });
                            }
                            browse
                        })
                        .inner;
                    if browse_library {
                        self.library_dialog.pick_directory();
                    }
                    self.download
                        .show(ui, &self.manifest, &self.library, app_id.ok());
                    self.download.show_import(ui, &self.libraries);
                    if self.download.job.is_none()
                        && let Ok(id) = app_id
                    {
                        self.show_steam_install(ui, id);
                    }
                });
            });
        });

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
    fn show_steam_install(&mut self, ui: &mut egui::Ui, app_id: u32) {
        ui.add_space(14.0);
        ui.separator();
        ui.heading("Steam 客户端安装");
        if self.installed.as_ref().is_none_or(|(id, _)| *id != app_id) {
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
                    ui.label(
                        "已发起安装请求。请在 Steam 中确认许可证和安装目录，完成后刷新 Steam 库。",
                    );
                }
                Err(error) => {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            }
        }
    }
}

#[derive(Default)]
struct DepotPanel {
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
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
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

    fn show(
        &mut self,
        ui: &mut egui::Ui,
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
                ui.colored_label(ui.visuals().error_fg_color, message);
            }
            None => {}
        }
        Self::show_progress(ui, &self.last_progress);
    }

    fn show_import(&mut self, ui: &mut egui::Ui, libraries: &[steam_library::Library]) {
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
                    ui.colored_label(ui.visuals().error_fg_color, error);
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

fn validate_app_id(input: &str) -> Result<u32, &'static str> {
    let input = input.trim();
    if input.is_empty() {
        return Err("");
    }
    if input.bytes().any(|byte| !byte.is_ascii_digit()) {
        return Err("请输入有效的 Steam AppID（1–4294967295）。");
    }
    match input.parse::<u32>() {
        Ok(value) if value > 0 => Ok(value),
        _ => Err("请输入有效的 Steam AppID（1–4294967295）。"),
    }
}

fn path_input(ui: &mut egui::Ui, label: &str, action: &str, value: &mut Option<PathBuf>) -> bool {
    ui.label(label);
    ui.horizontal(|ui| {
        let browse = ui.button(action).clicked();
        if value.is_some()
            && ui
                .button("清除")
                .on_hover_text(format!("清除{label}"))
                .clicked()
        {
            *value = None;
        }
        if let Some(path) = value {
            let display_path = path.to_string_lossy();
            ui.add(egui::Label::new(display_path.as_ref()).truncate())
                .on_hover_text(display_path.as_ref());
        } else {
            ui.weak("尚未选择");
        }
        browse
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::validate_app_id;

    #[test]
    fn accepts_positive_steam_app_ids() {
        assert_eq!(validate_app_id("440"), Ok(440));
        assert_eq!(validate_app_id(" 4294967295 "), Ok(u32::MAX));
    }

    #[test]
    fn rejects_empty_non_numeric_zero_and_overflow() {
        assert!(validate_app_id("").is_err());
        assert!(validate_app_id("abc").is_err());
        assert!(validate_app_id("0").is_err());
        assert!(validate_app_id("4294967296").is_err());
    }
}
