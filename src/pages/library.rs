use std::fs;

use eframe::egui::{self, RichText};

use crate::steam_library;
use crate::ui;
use crate::GameHaven;

impl GameHaven {
    pub(crate) fn library_page(&mut self, ui: &mut egui::Ui) {
        ui::page_header(ui, "游戏库", "已发现的 Steam 库与磁盘上的安装记录");

        ui.horizontal(|ui| {
            if ui.button("刷新").clicked() {
                self.libraries = steam_library::discover();
                self.installed = None;
            }
            ui.weak(format!("{} 个库", self.libraries.len()));
        });
        ui.add_space(10.0);

        if self.libraries.is_empty() {
            ui.label("没有发现 Steam 库。先启动 Steam 并完成初始化，再刷新。");
            return;
        }

        for library in &self.libraries {
            let count = installed_manifests(library);
            egui::Frame::group(ui.style())
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(library.client.name()).strong());
                        ui.weak(format!("{count} 个安装记录"));
                    });
                    ui.weak(library.path.display().to_string());
                });
            ui.add_space(6.0);
        }

        ui.add_space(4.0);
        ui.weak("游戏卡片、封面和 DLC 信息放到下一阶段，先把添加和修复链路做稳。");
    }
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
