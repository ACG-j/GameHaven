use std::path::PathBuf;

use eframe::egui::{Align, Layout, vec2};
use egui_mineral_material::prelude::{Select, SelectOption, TextField, TextFieldVariant};
use egui_mineral_material::{MaterialUi, icons};

use crate::DepotPanel;
use crate::GameHaven;
use crate::design::validate_app_id;
use crate::design::{OK, SPACE_1, SPACE_2, SPACE_3, SPACE_4};
use crate::sls;
use crate::steam;
use crate::widgets;

/// 搜索结果一次最多列出多少条，避免长列表把卡片撑高。
const SEARCH_RESULT_LIMIT: usize = 8;

impl GameHaven {
    pub(crate) fn add_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 第一步：按名称搜索或直接输入 AppID，再按需查询 Steam 商店。
        widgets::section_step(ui, 1, "选择游戏", |ui| {
            // 搜索行：输入井占满剩余宽度，按钮与回车都能发起商店搜索。
            let search_busy = self.search.busy();
            let mut submit = false;
            ui.horizontal(|ui| {
                let field_width = (ui.available_width() - 108.0).max(160.0);
                ui.allocate_ui_with_layout(vec2(field_width, 56.0), Layout::top_down(Align::Min), |ui| {
                    let field = ui.mineral(
                        TextField::new("add-search", "按名称搜索游戏", &mut self.search_term)
                            .placeholder("例如 空洞骑士")
                            .variant(TextFieldVariant::Outlined),
                    );
                    if field.output().submitted {
                        submit = true;
                    }
                });
                ui.add_space(SPACE_2);
                if widgets::tonal_button_icon(ui, "add-search-btn", "搜索", icons::ICON_SEARCH, !search_busy) {
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
                            .map(|item| (item.id, item.name.clone()))
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
                    for (id, name) in found {
                        if widgets::list_row(ui, &format!("add-search-{id}"), name, &format!("#{id}")) {
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
            let mut submitted = false;
            ui.allocate_ui_with_layout(vec2(280.0, 76.0), Layout::top_down(Align::Min), |ui| {
                let field = ui.mineral(
                    TextField::new("add-app-id", "Steam AppID", &mut self.app_id)
                        .placeholder("例如 440")
                        .variant(TextFieldVariant::Outlined),
                );
                submitted = field.output().submitted;
            });
            widgets::caption(
                ui,
                "Steam 商店页网址里的数字，例如 store.steampowered.com/app/440。",
            );

            // 输入过但无效时显示内联错误；帮助文字已由输入井的 supporting 承担。
            let validation = validate_app_id(&self.app_id);
            let valid = validation.ok();
            if let Err(message) = validation
                && !self.app_id.trim().is_empty()
                && !message.is_empty()
            {
                widgets::error_text(ui, message);
            }

            ui.add_space(SPACE_3);
            let busy = self.store_lookup.busy();
            let clicked = widgets::filled_button_icon(
                ui,
                "lookup-store",
                "查询商店",
                icons::ICON_SEARCH,
                valid.is_some() && !busy,
            );
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
        widgets::section_step(ui, 2, "写入 SLSsteam", |ui| {
            match steam::env::sls_config_path() {
                Some(path) => {
                    widgets::key_value(ui, "配置文件", &path.display().to_string());
                    ui.add_space(SPACE_4);

                    // 一键添加：查 appinfo、写入游戏与 DLC，再尝试从公开镜像拉 manifest；
                    // 跑完由 main.rs 按 auto_restart 决定是否重启 Steam。
                    ui.horizontal(|ui| {
                        if widgets::filled_button_icon(
                            ui,
                            "add-oneclick",
                            "一键添加",
                            icons::ICON_BOLT,
                            app_id.is_some() && self.add_game.is_none(),
                        ) && let Some(id) = app_id
                        {
                            self.start_add_game(id, ui.ctx());
                        }
                        if self.add_game.is_some() {
                            ui.add_space(SPACE_2);
                            ui.spinner();
                            widgets::caption(ui, "正在查询 appinfo、写入配置并拉取 manifest…");
                        }
                    });
                    ui.add_space(SPACE_1);
                    widgets::caption(
                        ui,
                        "一键添加会写入游戏与 DLC，并自动尝试从公开镜像下载 depot manifest；完成后按设置自动重启 Steam。",
                    );
                    ui.add_space(SPACE_4);

                    widgets::switch_row(
                        ui,
                        "auto-restart",
                        "写入后自动重启 Steam",
                        "写入成功后结束并重新启动 Steam，让 SLSsteam 立即生效。",
                        &mut self.auto_restart,
                    );
                    ui.add_space(SPACE_4);
                    // 写入动作会把商店列出的 DLC 一起带上，先在按钮前说明。
                    widgets::caption(
                        ui,
                        "会同时把商店列出的 DLC 写进 AdditionalApps 和 DlcData。",
                    );
                    ui.add_space(SPACE_2);
                    ui.horizontal(|ui| {
                        if widgets::tonal_button_icon(
                            ui,
                            "sls-add-app",
                            "加入游戏与 DLC",
                            icons::ICON_ADD_CIRCLE,
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
                    });
                }
                None => {
                    widgets::error_text(ui, "没有找到 SLSsteam 的 config.yaml。");
                    ui.add_space(SPACE_1);
                    widgets::caption(ui, "先安装 SLSsteam，并启动一次 Steam 让它生成配置。");
                }
            }
            // 上一次写入的结果。
            if let Some(result) = &self.add_result {
                widgets::result_banner(ui, result);
            }
        });

        // 第三步：本地清单与 Depot 下载，是可选的补充路径。
        widgets::section_step(ui, 3, "本地清单与 Depot 下载", |ui| {
            let busy = self.download.busy();

            let manifest_hint = match &self.manifest {
                Some(path) => path.display().to_string(),
                None => "尚未选择；只用 SLSsteam 也可以跳过这一步。".to_owned(),
            };
            widgets::control_row(ui, "本地清单", Some(&manifest_hint), |ui| {
                    ui.horizontal(|ui| {
                        if widgets::tonal_button_icon(
                            ui,
                            "pick-manifest",
                            "选择文件…",
                            icons::ICON_UPLOAD_FILE,
                            !busy,
                        ) {
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
            });
            ui.add_space(SPACE_3);
            let library_hint = match &self.library {
                Some(path) => path.display().to_string(),
                None => "尚未选择；Depot 下载需要目标库目录。".to_owned(),
            };
            widgets::control_row(ui, "目标库", Some(&library_hint), |ui| {
                    ui.horizontal(|ui| {
                        if widgets::tonal_button_icon(
                            ui,
                            "pick-library",
                            "选择目录…",
                            icons::ICON_FOLDER_OPEN,
                            !busy,
                        ) {
                            self.library_dialog.pick_directory();
                        }
                        if self.library.is_some()
                            && !busy
                            && widgets::text_button(ui, "clear-library", "清除")
                        {
                            self.library = None;
                        }
                    });
            });

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
                let select = ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), 56.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.mineral(
                            Select::optional("known-libraries", "已发现的 Steam 库", &mut picked)
                                .items(options),
                        )
                    },
                )
                .inner;
                if select.changed() && picked != current {
                    self.library = picked.map(PathBuf::from);
                }
            }

            // Depot 面板由 main.rs 提供：解析清单、下载，再复制到 Steam 库。
            self.download
                .show(ui, &self.manifest, &self.library, app_id);
            self.download.show_import(ui, &self.libraries);
        });

        // 第四步：用户自备的 manifest，用来把 depot 锁在固定版本上。
        widgets::section_step(ui, 4, "本地 manifest 与版本", |ui| {
            widgets::caption(
                ui,
                "文件名必须是 <depot>_<manifest>.manifest。导入后会写入 Steam 的 depotcache、写 SLSsteam 的 ManifestIds，并按当前 AppID 生成 ACF。",
            );

            // 文件选择行：左边挑文件，右边显示当前路径（超宽截断）。
            ui.add_space(SPACE_3);
            let manifest_state = match &self.depot_manifest {
                Some(path) => path.display().to_string(),
                None => "尚未选择".to_owned(),
            };
            widgets::control_row(ui, "版本清单", Some(&manifest_state), |ui| {
                if widgets::tonal_button_icon(
                    ui,
                    "add-depot-manifest",
                    "选择 .manifest",
                    icons::ICON_UPLOAD_FILE,
                    true,
                ) {
                    self.depot_manifest_dialog.pick_file();
                }
            });

            // 下载行：填 depot 与 manifest ID，从内置镜像取同名文件；
            // 下载成功后由 main.rs 填好 depot_manifest 并走与手动选择相同的导入流程。
            ui.add_space(SPACE_3);
            let download_busy = self.manifest_download.is_some();
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(140.0, 56.0), Layout::top_down(Align::Min), |ui| {
                    ui.mineral(
                        TextField::new("depot-id", "Depot ID", &mut self.depot_input)
                            .placeholder("例如 4401")
                            .variant(TextFieldVariant::Outlined),
                    );
                });
                ui.add_space(SPACE_2);
                ui.allocate_ui_with_layout(vec2(180.0, 56.0), Layout::top_down(Align::Min), |ui| {
                    ui.mineral(
                        TextField::new("manifest-id", "Manifest ID", &mut self.manifest_input)
                            .placeholder("例如 8744596894457938025")
                            .variant(TextFieldVariant::Outlined),
                    );
                });
                ui.add_space(SPACE_2);
                if widgets::tonal_button_icon(
                    ui,
                    "add-download-manifest",
                    "从仓库下载",
                    icons::ICON_DOWNLOAD,
                    !download_busy,
                ) {
                    self.start_manifest_download(ui.ctx());
                }
                if download_busy {
                    ui.add_space(SPACE_2);
                    ui.spinner();
                }
            });
            ui.add_space(SPACE_1);
            widgets::caption(
                ui,
                "内置 qwe213312 / mejikuhibiniu1 / Sainan 三个公开镜像，覆盖有限；找不到时用上面的「选择 .manifest」手动导入。",
            );

            // 解析只看文件名，所以每次重绘重新算一遍即可。
            let parsed = self
                .depot_manifest
                .as_deref()
                .and_then(steam::manifest::parse);
            if self.depot_manifest.is_some() {
                ui.add_space(SPACE_1);
                match &parsed {
                    Some(file) => widgets::caption(
                        ui,
                        &format!("Depot {} · Manifest {}", file.depot_id, file.manifest_id),
                    ),
                    None => widgets::error_text(ui, "文件名不是 <depot>_<manifest>.manifest"),
                }
            }

            // 解析成功才能导入：导入会落盘、写 ManifestIds 并生成 ACF。
            ui.add_space(SPACE_3);
            let ready = parsed.is_some();
            if widgets::filled_button_icon(
                ui,
                "add-import-manifest",
                "导入并锁定",
                icons::ICON_KEY,
                ready,
            ) {
                self.import_depot_manifest(ui.ctx());
            }

            // 上一次导入的结果。
            if let Some(result) = &self.depot_manifest_result {
                widgets::result_banner(ui, result);
            }
        });

        // Steam 客户端安装入口；只在 AppID 有效时显示。
        if let Some(id) = app_id {
            self.show_steam_install(ui, id);
        }
    }
}
