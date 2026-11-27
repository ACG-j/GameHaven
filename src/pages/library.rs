use eframe::egui::{self, Align, Layout, RichText};
use egui_mineral_material::MaterialUi;
use egui_mineral_material::prelude::{Card, CardVariant};

use crate::GameHaven;
use crate::design;
use crate::sls;
use crate::steam;
use crate::steam_library;
use crate::widgets;

impl GameHaven {
    pub(crate) fn library_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 顶部工具行：左侧库数量，右侧刷新（M3 里操作放在行尾）。
        let count = self.libraries.len();
        let mut refresh = false;
        ui.horizontal(|ui| {
            widgets::caption(ui, &format!("{count} 个库"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                refresh = widgets::tonal_button(ui, "library-refresh", "刷新", true);
            });
        });
        if refresh {
            // 重新扫描 Steam 目录，同时丢掉旧的安装位置缓存。
            self.libraries = steam_library::discover();
            self.installed = None;
        }
        ui.add_space(design::SPACE_4);

        // 注入集合每帧读一次：config.yaml 很小，不做额外缓存，点「刷新」后再读就是最新状态。
        let injected: std::collections::HashSet<u32> = steam::env::sls_config_path()
            .map(|path| sls::config::Config::new(path).apps().into_iter().collect())
            .unwrap_or_default();

        let ctx = ui.ctx().clone();
        // 先克隆库列表：遍历时不借用 self，后面才能调用 &mut self 的移除方法。
        let libraries = self.libraries.clone();
        let mut remove: Option<u32> = None;
        if libraries.is_empty() {
            widgets::empty_state(
                ui,
                "没有发现 Steam 库",
                "请先启动 Steam 并完成初始化，再回到这里点「刷新」重新扫描。",
            );
        } else {
            for library in &libraries {
                if let Some(app_id) = library_card(ui, library, &injected) {
                    remove = Some(app_id);
                }
                ui.add_space(design::SPACE_3);
            }
        }

        // 二次确认后才真正写配置；结果在同一帧就显示在列表下方。
        if let Some(app_id) = remove {
            self.remove_from_slssteam(app_id, &ctx);
        }

        // 移除结果放在列表之后、说明行之前，不遮挡游戏列表。
        if let Some(result) = &self.remove_result {
            match result {
                Ok(message) => {
                    ui.add_space(design::SPACE_2);
                    ui.label(RichText::new(message.as_str()).size(design::TEXT_BODY));
                }
                Err(message) => {
                    ui.add_space(design::SPACE_2);
                    widgets::error_text(ui, message.as_str());
                }
            }
        }

        // 底部说明：游戏卡片与封面留到下一阶段。
        ui.add_space(design::SPACE_2);
        widgets::caption(
            ui,
            "游戏卡片、封面与 DLC 信息放到下一阶段，当前先保证添加与修复链路可用。",
        );
        // 与设置页一致，滚到底部留一点呼吸空间。
        ui.add_space(design::SPACE_3);
    }
}

/// 单个库的卡片：客户端类型、安装记录数、库路径与游戏列表。
///
/// 返回值是用户在行内二次确认后要求移除的 AppID。
fn library_card(
    ui: &mut MaterialUi<'_>,
    library: &steam_library::Library,
    injected: &std::collections::HashSet<u32>,
) -> Option<u32> {
    // 每次重建卡片都重读 appmanifest，点「刷新」后列表就是最新的。
    let apps = steam_library::installed_apps(library);
    let count = apps.len();
    let client = library.client.name();
    let path = library.path.display().to_string();
    let width = ui.available_width();
    let mut remove = None;

    Card::new(format!("library-{path}"), format!("{client} · {path}"))
        .variant(CardVariant::Filled)
        .width(egui::Rangef::new(width, width))
        .content_padding(Card::CONTENT_PADDING)
        .show(ui, |ui| {
            // 第一行：客户端类型靠左，安装记录数靠右。
            ui.horizontal(|ui| {
                ui.label(RichText::new(client).size(design::TEXT_TITLE).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let color = design::primary(ui);
                    widgets::status_chip(ui, &format!("{count} 个安装记录"), color);
                });
            });
            ui.add_space(design::SPACE_1);
            widgets::caption(ui, &path);

            // 游戏数量与列表；数据来自 appmanifest，不额外缓存。
            ui.add_space(design::SPACE_2);
            widgets::caption(ui, &format!("{count} 个游戏"));
            ui.add_space(design::SPACE_2);
            if apps.is_empty() {
                widgets::caption(ui, "这个库还没有 appmanifest 记录");
            } else {
                for (index, app) in apps.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(design::SPACE_1);
                    }
                    if let Some(app_id) = game_row(ui, app, injected.contains(&app.app_id)) {
                        remove = Some(app_id);
                    }
                }
            }
        });

    remove
}

/// 单行游戏：左侧名称，右侧注入标记、移除操作、AppID 与占用空间。
///
/// 确认状态按行保存在 egui 内存里；返回值是二次确认后要求移除的 AppID。
fn game_row(
    ui: &mut MaterialUi<'_>,
    app: &steam_library::InstalledApp,
    injected: bool,
) -> Option<u32> {
    let name = if app.name.is_empty() {
        format!("App {}", app.app_id)
    } else {
        app.name.clone()
    };
    let mut meta = format!("#{} · {}", app.app_id, format_size(app.size_on_disk));
    let incomplete = app.size_on_disk == 0;
    if incomplete {
        meta.push_str(" · 未完成");
    }
    let variant = if incomplete {
        design::WARN
    } else {
        design::on_surface_variant(ui)
    };

    // key 用当前 scope 拼上 AppID：每个库的卡片各有 scope，同一库内不同 AppID 也互不影响。
    let confirm_id = ui.scope_id().with(("remove-confirm", app.app_id));
    let ctx = ui.ctx().clone();
    let mut confirm = ctx
        .data(|data| data.get_temp::<bool>(confirm_id))
        .unwrap_or(false);
    // 已经不在注入集合里时，残留的确认状态直接丢掉。
    if !injected {
        confirm = false;
    }
    // 本帧开始时是否已处于确认状态：点「移除」的那一帧只置位，确认面板留到下一帧。
    let show_confirm = confirm;

    let mut remove = None;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(meta).size(design::TEXT_LABEL).color(variant));
            if injected {
                // 点击「已注入」进入移除确认，避免另加按钮撑高列表行。
                let color = design::primary(ui);
                if widgets::action_chip(ui, "已注入", color) {
                    confirm = true;
                }
            }
            // 名称占剩余宽度，过长时截断，悬停可看到完整名称。
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let label = egui::Label::new(RichText::new(name.as_str()).size(design::TEXT_BODY))
                    .truncate();
                ui.add(label);
            });
        });
    });

    // 二次确认：只有上一帧点过「移除」才展开，取消与确认都回到未确认状态。
    if injected && show_confirm {
        ui.add_space(design::SPACE_1);
        ui.indent(format!("remove-confirm-panel-{}", app.app_id), |ui| {
            ui.label(RichText::new("确认从 SLSsteam 配置移除？").size(design::TEXT_BODY));
            ui.add_space(design::SPACE_2);
            ui.horizontal(|ui| {
                if widgets::filled_button(
                    ui,
                    &format!("remove-confirm-{}", app.app_id),
                    "确认移除",
                    true,
                ) {
                    confirm = false;
                    remove = Some(app.app_id);
                }
                ui.add_space(design::SPACE_2);
                if widgets::text_button(ui, &format!("remove-cancel-{}", app.app_id), "取消") {
                    confirm = false;
                }
            });
        });
    }

    ctx.data_mut(|data| {
        *data.get_temp_mut_or_default::<bool>(confirm_id) = confirm;
    });

    remove
}

/// 把字节数格式化成 Steam 风格的 MB / GB。
fn format_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let bytes = bytes as f64;
    if bytes >= 1024.0 * MB {
        format!("{:.1} GB", bytes / (1024.0 * MB))
    } else {
        format!("{} MB", (bytes / MB).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn formats_size_in_mb_and_gb() {
        assert_eq!(format_size(0), "0 MB");
        assert_eq!(format_size(1024 * 1024), "1 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    }
}
