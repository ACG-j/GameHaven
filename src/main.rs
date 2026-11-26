use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use eframe::egui::{self, CentralPanel, Frame, RichText, ScrollArea, TextEdit};
use egui_file_dialog::{FileDialog, FileDialogLabels};

mod depot;
mod fonts;
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
                            path_input(ui, "目标库", "选择目录…", &mut self.library)
                        })
                        .inner;
                    if browse_library {
                        self.library_dialog.pick_directory();
                    }
                    self.download
                        .show(ui, &self.manifest, &self.library, app_id.ok());
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

#[derive(Default)]
struct DepotPanel {
    job: Option<depot::Job>,
    plan: Option<depot::Plan>,
    selected: usize,
    result: Option<Result<&'static str, String>>,
    last_progress: depot::Progress,
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
            Ok(depot::Outcome::Downloaded) => self.result = Some(Ok("Depot 下载完成。")),
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
