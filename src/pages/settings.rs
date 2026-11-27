use egui_mineral_material::MaterialUi;

use crate::GameHaven;
use crate::design;
use crate::steam;
use crate::widgets;

impl GameHaven {
    pub(crate) fn settings_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 启动行为：写入配置后是否自动重启 Steam。
        widgets::section(ui, "启动行为", |ui| {
            widgets::switch(
                ui,
                "settings-auto-restart",
                "写入 SLSsteam 配置后自动重启 Steam",
                &mut self.auto_restart,
            );
            ui.add_space(design::SPACE_2);
            widgets::caption(
                ui,
                "重启会结束当前 Steam 进程，正在进行的下载与游戏会被中断。",
            );
        });

        // 路径：先取成字符串，避免闭包里同时借用 self 的多个字段。
        let steam_root = self
            .libraries
            .first()
            .map(|library| library.path.display().to_string());
        let sls_lib = steam::env::sls_so_path().map(|path| path.display().to_string());
        let sls_config = steam::env::sls_config_path().map(|path| path.display().to_string());
        widgets::section(ui, "路径", |ui| {
            path_row(ui, "Steam", steam_root.as_deref());
            path_row(ui, "SLSsteam", sls_lib.as_deref());
            path_row(ui, "配置", sls_config.as_deref());
        });

        // 关于：版本与许可证。
        widgets::section(ui, "关于", |ui| {
            widgets::key_value(ui, "版本", env!("CARGO_PKG_VERSION"));
            ui.add_space(design::SPACE_2);
            widgets::key_value(ui, "许可证", "GPL-3.0-or-later");
            ui.add_space(design::SPACE_2);
            widgets::caption(
                ui,
                "游栖 · GameHaven 以 GPL-3.0-or-later 发布，可自由使用、修改与再分发。",
            );
        });
    }
}

/// 一行路径；缺失时给出明确提示，避免整行消失让布局跳动。
fn path_row(ui: &mut MaterialUi<'_>, label: &str, value: Option<&str>) {
    widgets::key_value(ui, label, value.unwrap_or("未找到"));
    ui.add_space(design::SPACE_2);
}
