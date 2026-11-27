use eframe::egui;
use egui_mineral_material::MaterialUi;

use crate::DepotPanel;
use crate::sls;
use crate::steam;
use crate::ui;
use crate::GameHaven;

impl GameHaven {
    pub(crate) fn add_page(&mut self, ui: &mut MaterialUi<'_>) {
        ui::page_header(ui, "添加游戏", "把 AppID 写进 SLSsteam，让 Steam 认账后原生下载");

        ui::section(ui, "1 · 选择游戏", |ui| {
            let label = ui.label("Steam AppID");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.app_id)
                        .hint_text("输入 Steam AppID")
                        .desired_width(200.0),
                )
                .labelled_by(label.id);
                let app_id = ui::validate_app_id(&self.app_id);
                let busy = self.store_lookup.busy();
                if ui
                    .add_enabled(app_id.is_ok() && !busy, egui::Button::new("查询商店"))
                    .clicked()
                    && let Ok(app_id) = app_id
                {
                    self.store_lookup.start(app_id, ui.ctx());
                }
            });
            if !self.app_id.trim().is_empty()
                && let Err(message) = ui::validate_app_id(&self.app_id)
            {
                let error_color = ui.visuals().error_fg_color;
                ui.colored_label(error_color, message);
            }
            self.store_lookup
                .show(ui, ui::validate_app_id(&self.app_id).ok());
        });

        let app_id = ui::validate_app_id(&self.app_id).ok();

        ui::section(ui, "2 · 写入 SLSsteam", |ui| {
            match steam::env::sls_config_path() {
                Some(path) => {
                    ui.weak(format!("配置：{}", path.display()));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(app_id.is_some(), egui::Button::new("加入 AdditionalApps"))
                            .clicked()
                            && let Some(id) = app_id
                        {
                            self.add_to_slssteam(id, ui.ctx());
                        }
                        ui.checkbox(&mut self.auto_restart, "写入后重启 Steam");
                    });
                    if let Some(id) = app_id
                        && sls::config::Config::new(&path).has_app(id)
                    {
                        ui.add_space(4.0);
                        ui.weak(format!("{id} 已经在 AdditionalApps 里。"));
                    }
                }
                None => {
                    ui.label("没有找到 SLSsteam 的 config.yaml。");
                    ui.weak("先安装 SLSsteam，并启动一次 Steam 让它生成配置。");
                }
            }
            match &self.add_result {
                Some(Ok(message)) => {
                    ui.add_space(4.0);
                    ui.label(message);
                }
                Some(Err(message)) => {
                    ui.add_space(4.0);
                    let error_color = ui.visuals().error_fg_color;
                    ui.colored_label(error_color, message);
                }
                None => {}
            }
        });

        ui::section(ui, "3 · 本地清单与 Depot 下载（可选）", |ui| {
            let had_manifest = self.manifest.is_some();
            let browse_manifest = ui
                .add_enabled_ui(!self.download.busy(), |ui| {
                    ui::path_input(ui, "本地清单", "选择文件…", &mut self.manifest)
                })
                .inner;
            if browse_manifest {
                self.manifest_dialog.pick_file();
            }
            if had_manifest && self.manifest.is_none() {
                self.download = DepotPanel::default();
            }
            ui.add_space(8.0);
            let browse_library = ui
                .add_enabled_ui(!self.download.busy(), |ui| {
                    let browse = ui::path_input(ui, "目标库", "选择目录…", &mut self.library);
                    if !self.libraries.is_empty() {
                        ui.menu_button("已发现的 Steam 库", |ui| {
                            for library in &self.libraries {
                                let label = format!(
                                    "{} · {}",
                                    library.client.name(),
                                    library.path.display()
                                );
                                if ui.button(label).clicked() {
                                    self.library = Some(library.path.clone());
                                    ui.close();
                                }
                            }
                        });
                    }
                    browse
                })
                .inner;
            if browse_library {
                self.library_dialog.pick_directory();
            }
            self.download
                .show(ui, &self.manifest, &self.library, app_id);
            self.download.show_import(ui, &self.libraries);
        });

        if let Some(id) = app_id {
            self.show_steam_install(ui, id);
        }
    }
}
