use std::fs;

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

/// 单个库的卡片：客户端类型、安装记录数与库路径。
fn library_card(ui: &mut MaterialUi<'_>, library: &steam_library::Library) {
    let count = installed_manifests(library);
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
        });
}

fn installed_manifests(library: &steam_library::Library) -> usize {
    fs::read_dir(library.path.join("steamapps"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    name.starts_with("appmanifest_") && name.ends_with(".acf")
                })
                .count()
        })
        .unwrap_or(0)
}
