use eframe::egui::{self, RichText};

use crate::steam;
use crate::ui;
use crate::GameHaven;
use crate::RestartState;

impl GameHaven {
    pub(crate) fn environment_page(&mut self, ui: &mut egui::Ui) {
        ui::page_header(ui, "环境", "Steam、SLSsteam 与注入状态");

        ui.horizontal(|ui| {
            if let Some((ok, warn, error)) = self.env.summary() {
                ui.label(RichText::new(format!("{ok} 项正常")).color(ui::OK));
                if warn > 0 {
                    ui.label(RichText::new(format!("{warn} 项注意")).color(ui::WARN));
                }
                if error > 0 {
                    ui.label(
                        RichText::new(format!("{error} 项异常"))
                            .color(ui.visuals().error_fg_color),
                    );
                }
            } else {
                ui.spinner();
                ui.label("正在检查…");
            }
            if ui.button("重新检查").clicked() {
                self.env.refresh(ui.ctx());
            }
        });
        ui.add_space(8.0);
        self.env.show_checks(ui);
        ui.add_space(8.0);

        ui::section(ui, "维护", |ui| {
            ui.horizontal(|ui| {
                let busy = matches!(self.restart, RestartState::Running(_));
                if ui
                    .add_enabled(!busy, egui::Button::new("重启 Steam（带注入）"))
                    .clicked()
                {
                    self.start_restart(ui.ctx());
                }
                if busy {
                    ui.spinner();
                    ui.label("正在结束并重新启动 Steam…");
                }
            });
            ui.weak("重启会结束当前 Steam 进程，请先结束正在进行的下载或游戏。");
            match &self.restart {
                RestartState::Done(Ok(message)) => {
                    ui.add_space(4.0);
                    ui.label(message);
                }
                RestartState::Done(Err(message)) => {
                    ui.add_space(4.0);
                    ui.colored_label(ui.visuals().error_fg_color, message);
                }
                _ => {}
            }
        });

        let root = self.libraries.first().map(|library| library.path.clone());
        ui::section(ui, "路径", |ui| {
            if let Some(root) = &root {
                path_line(ui, "Steam", root);
            }
            if let Some(path) = steam::env::sls_so_path() {
                path_line(ui, "SLSsteam", &path);
            }
            if let Some(path) = steam::env::sls_config_path() {
                path_line(ui, "配置", &path);
            }
        });
    }
}

fn path_line(ui: &mut egui::Ui, label: &str, path: &std::path::Path) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        ui.weak(path.display().to_string());
    });
}
