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

/// 搜索结果一次最多列出多少条，避免长列表把卡片撑高。
const SEARCH_RESULT_LIMIT: usize = 8;

impl GameHaven {
    pub(crate) fn add_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 第一步：按名称搜索或直接输入 AppID，再按需查询 Steam 商店。
        widgets::section(ui, "1 · 选择游戏", |ui| {
            // 搜索行：按钮和回车都能发起商店搜索，查询期间禁用按钮。
            let search_busy = self.search.busy();
            let mut submit = false;
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.search_term)
                        .hint_text("按名称搜索游戏")
                        .desired_width(240.0),
                );
                // 单行输入框按回车会交出焦点，据此判定提交。
                if response.lost_focus()
                    && ui.ctx().input(|input| input.key_pressed(egui::Key::Enter))
                {
                    submit = true;
                }
                ui.add_space(SPACE_2);
                if widgets::tonal_button(ui, "add-search", "搜索", !search_busy) {
                    submit = true;
                }
                if search_busy {
                    ui.add_space(SPACE_2);
                    ui.spinner();
                }
            });
            if submit && !search_busy {
                let term = self.search_term.clone();
                self.search.start(term, ui.ctx());
            }

            // 搜索结果：最多列 8 条，点一条就填好 AppID 并查商店。
            let mut search_error: Option<String> = None;
            let mut candidates: Option<Vec<(u32, String)>> = None;
            match self.search.result() {
                Some(Ok(found)) => {
                    candidates = Some(
                        found
                            .iter()
                            .take(SEARCH_RESULT_LIMIT)
                            .map(|item| (item.id, format!("{} · {}", &item.name, item.id)))
                            .collect(),
                    );
                }
                Some(Err(message)) => search_error = Some(message.clone()),
                None => {}
            }

            let mut picked: Option<u32> = None;
            if let Some(message) = &search_error {
                ui.add_space(SPACE_2);
                widgets::error_text(ui, message);
            } else if let Some(found) = &candidates {
                ui.add_space(SPACE_2);
                if found.is_empty() {
                    widgets::caption(ui, "没有匹配的游戏");
                } else {
                    for (id, label) in found {
                        if widgets::text_button(ui, &format!("add-search-{id}"), label) {
                            picked = Some(*id);
                        }
                    }
                }
            }
            // 选中后立刻按这条 AppID 查商店，DLC 数量随之可见。
            if let Some(id) = picked {
                self.app_id = id.to_string();
                self.store_lookup.start(id, ui.ctx());
            }

            ui.add_space(SPACE_3);
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
            // 商店查到了 DLC 就提前说明：写入时会一并登记。
            if let Some(id) = valid {
                let dlc_count = self.store_lookup.dlcs(id).len();
                if dlc_count > 0 {
                    ui.add_space(SPACE_1);
                    widgets::caption(
                        ui,
                        &format!("商店列出 {dlc_count} 个 DLC，写入时会一并登记。"),
                    );
                }
            }
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
                    // 写入动作会把商店列出的 DLC 一起带上，先在按钮前说明。
                    widgets::caption(
                        ui,
                        "会同时把商店列出的 DLC 写进 AdditionalApps 和 DlcData。",
                    );
                    ui.add_space(SPACE_2);
                    if widgets::filled_button(ui, "sls-add-app", "加入游戏与 DLC", app_id.is_some())
                        && let Some(id) = app_id
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
