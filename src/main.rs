use std::path::PathBuf;

use eframe::egui::{self, CentralPanel, Frame, RichText, ScrollArea, TextEdit};
use egui_file_dialog::{FileDialog, FileDialogLabels};

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
        Box::new(|_| Ok(Box::new(GameHaven::new()))),
    )
}

struct GameHaven {
    app_id: String,
    manifest: Option<PathBuf>,
    library: Option<PathBuf>,
    manifest_dialog: FileDialog,
    library_dialog: FileDialog,
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
            manifest: None,
            library: None,
            manifest_dialog: picker("manifest", labels.clone()),
            library_dialog: picker("library", labels),
        }
    }
}

impl eframe::App for GameHaven {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
                    ui.add(
                        TextEdit::singleline(&mut self.app_id)
                            .hint_text("输入 Steam AppID")
                            .desired_width(242.0),
                    )
                    .labelled_by(label.id);
                    if !self.app_id.trim().is_empty()
                        && let Err(message) = validate_app_id(&self.app_id)
                    {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                    }

                    ui.add_space(12.0);
                    if path_input(ui, "本地清单", "选择文件…", &mut self.manifest) {
                        self.manifest_dialog.pick_file();
                    }
                    ui.add_space(10.0);
                    if path_input(ui, "目标库", "选择目录…", &mut self.library) {
                        self.library_dialog.pick_directory();
                    }
                });

                ui.add_space(12.0);
                ui.weak("Steam 查询、下载与安装尚未接入。");
            });
        });

        self.manifest_dialog.update(ui.ctx());
        if let Some(path) = self.manifest_dialog.take_picked() {
            self.manifest = Some(path);
            ui.ctx().request_repaint();
        }
        self.library_dialog.update(ui.ctx());
        if let Some(path) = self.library_dialog.take_picked() {
            self.library = Some(path);
            ui.ctx().request_repaint();
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
