use eframe::egui::{Align, Layout, RichText};
use egui_mineral_material::MaterialUi;

use crate::GameHaven;
use crate::design;
use crate::journal;
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

        // Steam 账号：loginusers.vdf 列账号，config.vdf 的 AutoLoginUser 决定自动登录谁。
        // 每次绘制都重读，切换后下一帧就是新状态，不额外维护缓存。
        let account_root = self.libraries.first().map(|library| library.path.clone());
        let accounts = account_root
            .as_deref()
            .map(steam::accounts::list)
            .unwrap_or_default();
        let current_account = account_root.as_deref().and_then(steam::accounts::current);
        widgets::section(ui, "Steam 账号", |ui| {
            if account_root.is_none() {
                widgets::caption(ui, "没有找到 Steam 库，先启动一次 Steam 完成初始化。");
                return;
            }
            if accounts.is_empty() {
                widgets::caption(ui, "没有找到 loginusers.vdf，先登录一次 Steam");
                return;
            }

            // 先把 token 取出，避免 status_chip(ui, .., design::primary(ui)) 同时借用 ui。
            let variant = design::on_surface_variant(ui);
            let accent = design::primary(ui);

            let mut switch_to = None;
            for (index, account) in accounts.iter().enumerate() {
                if index > 0 {
                    ui.add_space(design::SPACE_3);
                }
                let is_current = current_account
                    .as_deref()
                    .is_some_and(|current| current.eq_ignore_ascii_case(&account.account_name));
                let persona = if account.persona_name.is_empty() {
                    account.account_name.as_str()
                } else {
                    account.persona_name.as_str()
                };
                let detail = format!("{} · {}", account.account_name, account.steam_id);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // 已经是自动登录账号，就不再给切换按钮。
                        if !is_current
                            && widgets::text_button(
                                ui,
                                &format!("account-{}", account.steam_id),
                                "切换",
                            )
                        {
                            switch_to = Some(account.account_name.clone());
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(persona).size(design::TEXT_BODY).strong(),
                                    );
                                    if account.most_recent {
                                        ui.add_space(design::SPACE_1);
                                        widgets::status_chip(ui, "最近使用", variant);
                                    }
                                    if is_current {
                                        ui.add_space(design::SPACE_1);
                                        widgets::status_chip(ui, "当前", accent);
                                    }
                                });
                                widgets::caption(ui, &detail);
                            });
                        });
                    });
                });
            }

            // 改写 config.vdf；重绘一次让「当前」标记与按钮在下一帧同步。
            if let Some(account) = switch_to {
                self.switch_account(account, ui.ctx());
                ui.ctx().request_repaint();
            }

            if let Some(result) = &self.account_result {
                ui.add_space(design::SPACE_2);
                match result {
                    Ok(message) => {
                        ui.label(
                            RichText::new(message.as_str())
                                .size(design::TEXT_BODY)
                                .color(design::OK),
                        );
                    }
                    Err(message) => widgets::error_text(ui, message),
                }
            }

            ui.add_space(design::SPACE_3);
            widgets::caption(
                ui,
                "切换会改写 loginusers.vdf 的 MostRecent（旧版 Steam 同时同步 AutoLoginUser），重启 Steam 后生效。",
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

        // 最近操作：journal 只追加，取最后几条帮助回查写过哪些文件。
        let entries = journal::tail(12);
        widgets::section(ui, "最近操作", |ui| {
            if entries.is_empty() {
                widgets::caption(ui, "还没有操作记录");
            } else {
                for (index, entry) in entries.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(design::SPACE_1);
                    }
                    let line = format!("{} · {}", journal::relative(entry.at), entry.message);
                    widgets::caption(ui, &line);
                }
            }
            ui.add_space(design::SPACE_2);
            widgets::caption(ui, "日志文件在 ~/.local/share/gamehaven/journal.log");
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
