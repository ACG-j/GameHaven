//! 「环境」页：体检汇总、检查项、重启维护与路径。

use eframe::egui::{self, RichText};
use egui_mineral_material::MaterialUi;

use crate::GameHaven;
use crate::RestartState;
use crate::design::{self, SPACE_2, SPACE_3, SPACE_5, TEXT_BODY};
use crate::steam;
use crate::widgets;

/// 数量为 0 的状态 pill 用弱化色，避免「0 项异常」抢眼。
fn count_color(count: usize, active: egui::Color32, ui: &MaterialUi<'_>) -> egui::Color32 {
    if count > 0 {
        active
    } else {
        design::on_surface_variant(ui)
    }
}

impl GameHaven {
    pub(crate) fn environment_page(&mut self, ui: &mut MaterialUi<'_>) {
        self.environment_summary(ui);
        ui.add_space(SPACE_5);
        self.env.show_checks(ui);
        ui.add_space(SPACE_5);
        self.environment_maintenance(ui);
        self.environment_paths(ui);
    }

    /// 顶部体检汇总：三个状态 pill，旁边是重新检查。
    fn environment_summary(&mut self, ui: &mut MaterialUi<'_>) {
        let summary = self.env.summary();
        ui.horizontal(|ui| {
            if let Some((ok, warn, error)) = summary {
                let ok_color = count_color(ok, design::OK, ui);
                let warn_color = count_color(warn, design::WARN, ui);
                let error_color = count_color(error, design::error(ui), ui);
                widgets::status_chip(ui, &format!("{ok} 项正常"), ok_color);
                ui.add_space(SPACE_2);
                widgets::status_chip(ui, &format!("{warn} 项注意"), warn_color);
                ui.add_space(SPACE_2);
                widgets::status_chip(ui, &format!("{error} 项异常"), error_color);
                ui.add_space(SPACE_3);
            } else {
                ui.spinner();
                ui.add_space(SPACE_2);
                widgets::caption(ui, "正在检查…");
                ui.add_space(SPACE_3);
            }
            // 检查未返回时禁用，避免重复启动后台线程。
            if widgets::tonal_button(ui, "env-refresh", "重新检查", summary.is_some()) {
                self.env.refresh(ui.ctx());
            }
        });
    }

    /// 维护卡片：重启 Steam 需要二次确认，运行期间只显示进度。
    fn environment_maintenance(&mut self, ui: &mut MaterialUi<'_>) {
        let busy = matches!(self.restart, RestartState::Running(_));
        let confirm_id = ui.scope_id().with("restart-confirm");
        let ctx = ui.ctx().clone();
        // 确认状态跨帧保存在 egui 内存里，属于页面本地状态。
        let mut confirm = ctx
            .data(|data| data.get_temp::<bool>(confirm_id))
            .unwrap_or(false);

        widgets::section(ui, "维护", |ui| {
            if busy {
                confirm = false;
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.add_space(SPACE_2);
                    ui.label(RichText::new("正在结束并重新启动 Steam…").size(TEXT_BODY));
                });
            } else if confirm {
                ui.label(
                    RichText::new("重启会结束当前 Steam 进程，进行中的下载和游戏会中断。")
                        .size(TEXT_BODY),
                );
                ui.add_space(SPACE_3);
                ui.horizontal(|ui| {
                    if widgets::filled_button(ui, "env-restart-confirm", "确认重启", true) {
                        confirm = false;
                        self.start_restart(ui.ctx());
                    }
                    ui.add_space(SPACE_2);
                    if widgets::text_button(ui, "env-restart-cancel", "取消") {
                        confirm = false;
                    }
                });
            } else {
                if widgets::tonal_button(ui, "env-restart", "重启 Steam", true) {
                    confirm = true;
                }
                ui.add_space(SPACE_2);
                widgets::caption(
                    ui,
                    "重启会结束当前 Steam 进程，请先结束正在进行的下载或游戏。",
                );

                // 上一次重启的结果，确认阶段先让位给确认文案。
                match &self.restart {
                    RestartState::Done(Ok(message)) => {
                        ui.add_space(SPACE_3);
                        ui.label(
                            RichText::new(message.as_str())
                                .size(TEXT_BODY)
                                .color(design::OK),
                        );
                    }
                    RestartState::Done(Err(message)) => {
                        ui.add_space(SPACE_3);
                        widgets::error_text(ui, message.as_str());
                    }
                    _ => {}
                }
            }
        });

        ctx.data_mut(|data| {
            *data.get_temp_mut_or_default::<bool>(confirm_id) = confirm;
        });
    }

    /// 路径卡片：Steam / SLSsteam / 配置。
    fn environment_paths(&self, ui: &mut MaterialUi<'_>) {
        let mut rows: Vec<(&'static str, String)> = Vec::new();
        if let Some(root) = self.libraries.first().map(|library| library.path.clone()) {
            rows.push(("Steam", root.display().to_string()));
        }
        if let Some(path) = steam::env::sls_so_path() {
            rows.push(("SLSsteam", path.display().to_string()));
        }
        if let Some(path) = steam::env::sls_config_path() {
            rows.push(("配置", path.display().to_string()));
        }

        widgets::section(ui, "路径", |ui| {
            if rows.is_empty() {
                widgets::caption(ui, "尚未发现 Steam 或 SLSsteam 的路径。");
                return;
            }
            for (index, (label, value)) in rows.iter().enumerate() {
                if index > 0 {
                    ui.add_space(SPACE_2);
                }
                widgets::key_value(ui, label, value);
            }
        });
    }
}
