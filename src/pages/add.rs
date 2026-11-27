use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText};
use egui_mineral_material::MaterialUi;
use egui_mineral_material::prelude::{Select, SelectOption, TextField, TextFieldVariant};

use crate::DepotPanel;
use crate::GameHaven;
use crate::design::{OK, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TEXT_BODY};
use crate::sls;
use crate::steam;
use crate::design::validate_app_id;
use crate::widgets;

impl GameHaven {
    pub(crate) fn add_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 第一步：输入并校验 AppID，可选地查询 Steam 商店。
        widgets::section(ui, "1 · 选择游戏", |ui| {
            let field = ui.mineral(
                TextField::new("add-app-id", "Steam AppID", &mut self.app_id)
                    .placeholder("例如 440")
                    .variant(TextFieldVariant::Outlined),
            );
            let submitted = field.output().submitted;

            // 输入过但无效时显示内联错误；其余情况显示一句帮助文字。
            let validation = validate_app_id(&self.app_id);
            let valid = validation.ok();
            match validation {
                Err(message) if !self.app_id.trim().is_empty() && !message.is_empty() => {
                    widgets::error_text(ui, message);
                }
                _ => widgets::caption(
                    ui,
                    "Steam 商店页网址里的数字，例如 store.steampowered.com/app/440。",
                ),
            }

            ui.add_space(SPACE_3);
            let busy = self.store_lookup.busy();
            let clicked =
                widgets::filled_button(ui, "lookup-store", "查询商店", valid.is_some() && !busy);
            // 按钮和回车都能发起查询；AppID 无效时两者都不可用。
            if (clicked || submitted)
                && !busy
                && let Some(id) = valid
            {
                self.store_lookup.start(id, ui.ctx());
            }
            ui.add_space(SPACE_2);
            self.store_lookup.show(ui, valid);
        });

        // 输入框可能刚被编辑过，这里重新校验一次，供后面两步使用。
        let app_id = validate_app_id(&self.app_id).ok();

        // 第二步：把 AppID 写进 SLSsteam，可选择写入后自动重启 Steam。
        widgets::section(ui, "2 · 写入 SLSsteam", |ui| {
            match steam::env::sls_config_path() {
                Some(path) => {
                    widgets::key_value(ui, "配置文件", &path.display().to_string());
                    ui.add_space(SPACE_4);
                    widgets::switch(
                        ui,
                        "auto-restart",
                        "写入后自动重启 Steam",
                        &mut self.auto_restart,
                    );
                    ui.add_space(SPACE_1);
                    widgets::caption(ui, "写入成功后结束并重新启动 Steam，让 SLSsteam 立即生效。");
                    ui.add_space(SPACE_4);
                    if widgets::filled_button(
                        ui,
                        "sls-add-app",
                        "加入 AdditionalApps",
                        app_id.is_some(),
                    ) && let Some(id) = app_id
                    {
                        self.add_to_slssteam(id, ui.ctx());
                    }
                    if let Some(id) = app_id
                        && sls::config::Config::new(&path).has_app(id)
                    {
                        ui.add_space(SPACE_2);
                        widgets::status_chip(ui, &format!("{id} 已在 AdditionalApps 里"), OK);
                    }
                }
                None => {
                    widgets::error_text(ui, "没有找到 SLSsteam 的 config.yaml。");
                    ui.add_space(SPACE_1);
                    widgets::caption(ui, "先安装 SLSsteam，并启动一次 Steam 让它生成配置。");
                }
            }
            // 上一次写入的结果。
            match &self.add_result {
                Some(Ok(message)) => {
                    ui.add_space(SPACE_3);
                    ui.label(RichText::new(message.as_str()).size(TEXT_BODY).color(OK));
                }
                Some(Err(message)) => {
                    ui.add_space(SPACE_3);
                    widgets::error_text(ui, message);
                }
                None => {}
            }
        });

        // 第三步：本地清单与 Depot 下载，是可选的补充路径。
        widgets::section(ui, "3 · 本地清单与 Depot 下载", |ui| {
            let busy = self.download.busy();

            widgets::caption(ui, "本地清单");
            ui.add_space(SPACE_1);
            ui.horizontal(|ui| {
                if widgets::tonal_button(ui, "pick-manifest", "选择文件…", !busy) {
                    self.manifest_dialog.pick_file();
                }
                if self.manifest.is_some()
                    && !busy
                    && widgets::text_button(ui, "clear-manifest", "清除")
                {
                    // 换掉清单后，之前解析出的 Depot 结果不再适用。
                    self.manifest = None;
                    self.download = DepotPanel::default();
                }
            });
            ui.add_space(SPACE_1);
            match &self.manifest {
                Some(path) => path_label(ui, path),
                None => widgets::caption(ui, "尚未选择；只用 SLSsteam 也可以跳过这一步。"),
            }

            ui.add_space(SPACE_4);
            widgets::caption(ui, "目标库");
            ui.add_space(SPACE_1);
            ui.horizontal(|ui| {
                if widgets::tonal_button(ui, "pick-library", "选择目录…", !busy) {
                    self.library_dialog.pick_directory();
                }
                if self.library.is_some()
                    && !busy
                    && widgets::text_button(ui, "clear-library", "清除")
                {
                    self.library = None;
                }
            });
            ui.add_space(SPACE_1);
            match &self.library {
                Some(path) => path_label(ui, path),
                None => widgets::caption(ui, "尚未选择；Depot 下载需要目标库目录。"),
            }

            if !self.libraries.is_empty() {
                ui.add_space(SPACE_3);
                // 已发现的 Steam 库：选中即作为目标库。
                let current = self.library.as_ref().map(|path| path.display().to_string());
                let mut picked = current.clone();
                let options = self.libraries.iter().map(|library| {
                    (
                        library.path.display().to_string(),
                        SelectOption::new(format!(
                            "{} · {}",
                            library.client.name(),
                            library.path.display()
                        )),
                    )
                });
                let select = ui.mineral(
                    Select::optional("known-libraries", "已发现的 Steam 库", &mut picked)
                        .items(options),
                );
                if select.changed() && picked != current {
                    self.library = picked.map(PathBuf::from);
                }
            }

            // Depot 面板由 main.rs 提供：解析清单、下载，再复制到 Steam 库。
            self.download
                .show(ui, &self.manifest, &self.library, app_id);
            self.download.show_import(ui, &self.libraries);
        });

        // Steam 客户端安装入口；只在 AppID 有效时显示。
        if let Some(id) = app_id {
            self.show_steam_install(ui, id);
        }
    }
}

/// 路径标签：超宽时截断，悬停显示完整路径。
fn path_label(ui: &mut MaterialUi<'_>, path: &Path) {
    let text = path.display().to_string();
    let response = ui.add(egui::Label::new(RichText::new(text.clone()).size(TEXT_BODY)).truncate());
    response.on_hover_text(text);
}
