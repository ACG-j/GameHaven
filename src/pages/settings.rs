use eframe::egui::{self, RichText};

use crate::steam;
use crate::ui;
use crate::GameHaven;

impl GameHaven {
    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        ui::page_header(ui, "设置", "启动行为与路径信息");

        ui::section(ui, "启动行为", |ui| {
            ui.checkbox(&mut self.auto_restart, "写入 SLSsteam 配置后自动重启 Steam");
            ui.weak("重启会结束当前 Steam 进程。");
        });

        ui::section(ui, "路径", |ui| {
            if let Some(root) = self.libraries.first().map(|library| library.path.clone()) {
                row(ui, "Steam", &root.display().to_string());
            }
            if let Some(path) = steam::env::sls_so_path() {
                row(ui, "SLSsteam", &path.display().to_string());
            }
            if let Some(path) = steam::env::sls_config_path() {
                row(ui, "配置", &path.display().to_string());
            }
        });

        ui::section(ui, "关于", |ui| {
            row(ui, "版本", env!("CARGO_PKG_VERSION"));
            ui.weak("游栖 · GameHaven");
        });
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        ui.weak(value);
    });
}
