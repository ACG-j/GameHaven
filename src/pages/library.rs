use eframe::egui::{self, Align, Layout, RichText};
use egui_mineral_material::MaterialUi;
use egui_mineral_material::prelude::{Card, CardVariant};

use crate::GameHaven;
use crate::design;
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

        if self.libraries.is_empty() {
            widgets::empty_state(
                ui,
                "没有发现 Steam 库",
                "请先启动 Steam 并完成初始化，再回到这里点「刷新」重新扫描。",
            );
        } else {
            for library in &self.libraries {
                library_card(ui, library);
                ui.add_space(design::SPACE_3);
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
fn library_card(ui: &mut MaterialUi<'_>, library: &steam_library::Library) {
    // 每次重建卡片都重读 appmanifest，点「刷新」后列表就是最新的。
    let apps = steam_library::installed_apps(library);
    let count = apps.len();
    let client = library.client.name();
    let path = library.path.display().to_string();
    let width = ui.available_width();

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
                    game_row(ui, app);
                }
            }
        });
}

/// 单行游戏：左侧名称，右侧 AppID 与占用空间。
fn game_row(ui: &mut MaterialUi<'_>, app: &steam_library::InstalledApp) {
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
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(meta).size(design::TEXT_LABEL).color(variant));
            // 名称占剩余宽度，过长时截断，悬停可看到完整名称。
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let label = egui::Label::new(RichText::new(name.as_str()).size(design::TEXT_BODY))
                    .truncate();
                ui.add(label);
            });
        });
    });
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
