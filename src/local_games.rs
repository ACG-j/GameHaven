use std::path::PathBuf;

use eframe::egui::{self, ComboBox, TextEdit};
use egui_file_dialog::{FileDialog, FileDialogLabels};

use crate::{path_input, shortcuts, steam_library::SteamAccount};

pub struct LocalGames {
    name: String,
    executable: Option<PathBuf>,
    directory: Option<PathBuf>,
    executable_picker: FileDialog,
    directory_picker: FileDialog,
    target: Option<SteamAccount>,
    result: Option<Result<&'static str, String>>,
}

impl LocalGames {
    pub fn new(mut labels: FileDialogLabels) -> Self {
        labels.title_select_file = "选择启动文件".into();
        labels.title_select_directory = "选择工作目录".into();
        let picker = |id, labels| {
            FileDialog::new()
                .id(egui::Id::unique(id))
                .labels(labels)
                .as_modal(true)
                .default_size([440.0, 270.0])
                .show_new_folder_button(false)
        };
        Self {
            name: String::new(),
            executable: None,
            directory: None,
            executable_picker: picker("local-executable", labels.clone()),
            directory_picker: picker("local-directory", labels),
            target: None,
            result: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, accounts: &[SteamAccount]) {
        ui.heading("添加本地游戏到 Steam");
        ui.add_space(10.0);
        if accounts.is_empty() {
            ui.label("没有发现 Steam 账号配置。请先在 Steam 中登录。");
            return;
        }
        if !self
            .target
            .as_ref()
            .is_some_and(|target| accounts.contains(target))
        {
            self.target = (accounts.len() == 1).then(|| accounts[0].clone());
        }
        ui.label("Steam 账号");
        ComboBox::from_id_salt("shortcut-account")
            .selected_text(
                self.target
                    .as_ref()
                    .map_or_else(|| "选择账号".to_owned(), account_label),
            )
            .show_ui(ui, |ui| {
                for account in accounts {
                    ui.selectable_value(
                        &mut self.target,
                        Some(account.clone()),
                        account_label(account),
                    )
                    .on_hover_text(account.config_dir.display().to_string());
                }
            });

        ui.add_space(8.0);
        let label = ui.label("游戏名称");
        ui.add(TextEdit::singleline(&mut self.name).desired_width(260.0))
            .labelled_by(label.id);
        ui.add_space(8.0);
        if path_input(ui, "启动文件", "选择文件…", &mut self.executable) {
            self.executable_picker.pick_file();
        }
        ui.add_space(8.0);
        if path_input(ui, "工作目录", "选择目录…", &mut self.directory) {
            self.directory_picker.pick_directory();
        }
        ui.add_space(10.0);

        let selection = self
            .target
            .as_ref()
            .zip(self.executable.as_ref())
            .zip(self.directory.as_ref());
        if ui
            .add_enabled(
                selection.is_some() && !self.name.trim().is_empty(),
                egui::Button::new("添加到 Steam"),
            )
            .clicked()
            && let Some(((account, executable), directory)) = selection
        {
            self.result = Some(
                shortcuts::add(&account.config_dir, &self.name, executable, directory).map(
                    |outcome| match outcome {
                        shortcuts::Import::Added => "快捷方式已保存。启动 Steam 后可在库中查看。",
                        shortcuts::Import::AlreadyExists => "这个快捷方式已在 Steam 库中。",
                    },
                ),
            );
        }
        if self
            .executable
            .as_ref()
            .and_then(|path| path.extension())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            ui.weak("在 Steam 的快捷方式属性中选择 Proton 运行 Windows 游戏。");
        }
        if let Some(result) = &self.result {
            match result {
                Ok(message) => {
                    ui.label(*message);
                }
                Err(message) => {
                    ui.colored_label(ui.visuals().error_fg_color, message);
                }
            }
        }
    }

    pub fn update_pickers(&mut self, ctx: &egui::Context) {
        self.executable_picker.update(ctx);
        if let Some(path) = self.executable_picker.take_picked() {
            self.directory = path.parent().map(PathBuf::from);
            if self.name.trim().is_empty()
                && let Some(name) = path.file_stem()
            {
                self.name = name.to_string_lossy().into_owned();
            }
            self.executable = Some(path);
            self.result = None;
            ctx.request_repaint();
        }
        self.directory_picker.update(ctx);
        if let Some(path) = self.directory_picker.take_picked() {
            self.directory = Some(path);
            self.result = None;
            ctx.request_repaint();
        }
    }
}

fn account_label(account: &SteamAccount) -> String {
    format!("{} · {}", account.client.name(), account.steam_id)
}
