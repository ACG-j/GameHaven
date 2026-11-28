use eframe::egui::{Align, Layout, RichText, vec2};
use egui_mineral_material::prelude::ButtonVariant;
use egui_mineral_material::{MaterialUi, icons};

use crate::GameHaven;
use crate::design;
use crate::journal;
use crate::steam;
use crate::widgets;

impl GameHaven {
    pub(crate) fn settings_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 启动行为：写入配置后是否自动重启 Steam。
        widgets::section(ui, icons::ICON_ROCKET_LAUNCH, "启动行为", |ui| {
            widgets::switch_row(
                ui,
                "settings-auto-restart",
                "写入 SLSsteam 配置后自动重启 Steam",
                "重启会结束当前 Steam 进程，正在进行的下载与游戏会被中断。",
                &mut self.auto_restart,
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
        widgets::section(ui, icons::ICON_MANAGE_ACCOUNTS, "Steam 账号", |ui| {
            if account_root.is_none() {
                widgets::caption(ui, "没有找到 Steam 库，先启动一次 Steam 完成初始化。");
                return;
            }
            if accounts.is_empty() {
                widgets::caption(ui, "没有找到 loginusers.vdf，先登录一次 Steam");
                return;
            }

            let accent = design::primary(ui);
            let variant = design::on_surface_variant(ui);

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
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), 72.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                    widgets::avatar(ui, persona.chars().next().unwrap_or('?'), 40.0);
                    ui.add_space(design::SPACE_3);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(persona).size(design::TEXT_BODY).strong());
                            if account.most_recent {
                                ui.add_space(design::SPACE_1);
                                widgets::status_chip(ui, "最近使用", variant);
                            }
                        });
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(&detail)
                                .font(design::mono(design::TEXT_LABEL))
                                .color(variant),
                        );
                    });
                    // 行尾固定槽位：当前标记或切换按钮，逐行对齐。
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.allocate_ui_with_layout(vec2(76.0, 40.0), Layout::right_to_left(Align::Center), |ui| {
                            if is_current {
                                widgets::status_chip(ui, "当前", accent);
                            } else if widgets::row_button(
                                ui,
                                &format!("account-{}", account.steam_id),
                                "切换",
                                None,
                                ButtonVariant::Tonal,
                                true,
                            ) {
                                switch_to = Some(account.account_name.clone());
                            }
                        });
                    });
                    },
                );
            }

            // 改写 config.vdf；重绘一次让「当前」标记与按钮在下一帧同步。
            if let Some(account) = switch_to {
                self.switch_account(account, ui.ctx());
                ui.ctx().request_repaint();
            }

            if let Some(result) = &self.account_result {
                widgets::result_banner(ui, result);
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
        widgets::section(ui, icons::ICON_FOLDER_OPEN, "路径", |ui| {
            widgets::path_row(ui, "Steam", steam_root.as_deref());
            ui.add_space(design::SPACE_2);
            widgets::path_row(ui, "SLSsteam", sls_lib.as_deref());
            ui.add_space(design::SPACE_2);
            widgets::path_row(ui, "配置", sls_config.as_deref());
        });

        // 撤销：写操作前备份到 undo 目录，这里恢复最近一次。
        let undo_info = journal::undo_info();
        widgets::section(ui, icons::ICON_RESTORE, "撤销", |ui| {
            let Some(info) = undo_info.as_deref() else {
                widgets::caption(ui, "没有可撤销的操作");
                return;
            };
            widgets::key_value(ui, "最近一次写操作", info);
            ui.add_space(design::SPACE_3);

            let confirm_id = ui.scope_id().with("undo-confirm");
            let mut confirm = ui
                .ctx()
                .data(|data| data.get_temp::<bool>(confirm_id))
                .unwrap_or(false);
            if confirm {
                ui.label(
                    RichText::new("确认恢复最近一次写操作前的文件？").size(design::TEXT_BODY),
                );
                ui.add_space(design::SPACE_2);
                ui.horizontal(|ui| {
                    if widgets::filled_button(ui, "settings-undo-confirm", "确认撤销", true) {
                        self.undo_last_write(ui.ctx());
                        confirm = false;
                    }
                    ui.add_space(design::SPACE_2);
                    if widgets::text_button(ui, "settings-undo-cancel", "取消") {
                        confirm = false;
                    }
                });
            } else if widgets::tonal_button_icon(
                ui,
                "settings-undo",
                "撤销最近一次写操作",
                icons::ICON_RESTORE,
                true,
            ) {
                confirm = true;
            }
            ui.ctx().data_mut(|data| data.insert_temp(confirm_id, confirm));

            if let Some(result) = &self.undo_result {
                widgets::result_banner(ui, result);
            }

            ui.add_space(design::SPACE_3);
            widgets::caption(
                ui,
                "写操作前会把原文件复制到 ~/.local/share/gamehaven/undo，撤销恢复最近一次。",
            );
        });

        // 最近操作：journal 只追加，取最后几条帮助回查写过哪些文件。
        let entries = journal::tail(12);
        widgets::section(ui, icons::ICON_HISTORY, "最近操作", |ui| {
            if entries.is_empty() {
                widgets::caption(ui, "还没有操作记录");
            } else {
                for (index, entry) in entries.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(design::SPACE_1);
                    }
                    widgets::journal_row(ui, &journal::relative(entry.at), &entry.message);
                }
            }
            ui.add_space(design::SPACE_3);
            widgets::caption(ui, "日志文件在 ~/.local/share/gamehaven/journal.log");
        });

        // 成就：SLScheevo 是可选外部工具，只做检测，不代管。
        let scheevo = steam::env::slscheevo_path();
        widgets::section(ui, icons::ICON_EMOJI_EVENTS, "成就", |ui| {
            match scheevo.as_deref() {
                Some(path) => {
                    widgets::key_value(ui, "SLScheevo", &path.display().to_string());
                    ui.add_space(design::SPACE_2);
                    widgets::caption(
                        ui,
                        "已检测到 SLScheevo；成就数据的生成与导入在它自己的界面里完成。",
                    );
                }
                None => {
                    widgets::caption(ui, "没有找到 SLScheevo（可选）。");
                    ui.add_space(design::SPACE_1);
                    widgets::caption(
                        ui,
                        "它是独立的成就数据生成工具，放在 ~/.local/bin 或 /usr/local/bin 即可被识别。",
                    );
                }
            }
        });

        // 关于：版本、更新检查与许可证。
        widgets::section(ui, icons::ICON_INFO, "关于", |ui| {
            widgets::key_value(ui, "版本", env!("CARGO_PKG_VERSION"));
            ui.add_space(design::SPACE_3);
            let busy = self.update_check.is_some();
            ui.horizontal(|ui| {
                if widgets::tonal_button_icon(
                    ui,
                    "settings-check-update",
                    "检查更新",
                    icons::ICON_UPDATE,
                    !busy,
                ) {
                    self.check_update(ui.ctx());
                }
                if busy {
                    ui.add_space(design::SPACE_2);
                    ui.spinner();
                    widgets::caption(ui, "正在检查 GitHub Releases…");
                }
            });
            if let Some(result) = &self.update_result {
                widgets::result_banner(ui, result);
            }
            ui.add_space(design::SPACE_3);
            widgets::key_value(ui, "许可证", "GPL-3.0-or-later");
            ui.add_space(design::SPACE_2);
            widgets::caption(
                ui,
                "游栖 · GameHaven 以 GPL-3.0-or-later 发布，可自由使用、修改与再分发。",
            );
        });
    }
}