use eframe::egui::{self, Align, Align2, CornerRadius, FontId, Layout, Rect, RichText, Sense, vec2};
use egui_mineral_material::prelude::{Card, CardVariant, IconButtonVariant, MaterialExt};
use egui_mineral_material::{MaterialUi, icons};

use crate::GameHaven;
use crate::covers;
use crate::design::{self, ROW_HEIGHT, tint};
use crate::sls;
use crate::steam;
use crate::steam_library;
use crate::widgets;

/// 游戏行产生的操作。
///
/// `game_row` 是自由函数，拿不到 `&mut GameHaven`，所以行内只返回动作，由页面层收集后统一下发。
/// 「启动」需要知道走哪个客户端，因此带上库索引。
#[derive(Clone)]
enum RowAction {
    /// 二次确认后移除这个 AppID 的注入。
    Remove(u32),
    /// 用 `libraries[library]` 对应的客户端启动 `app_id`。
    Launch { library: usize, app_id: u32 },
    /// 按实际目录大小修复这个 AppID 的 ACF。
    Repair {
        library: usize,
        app_id: u32,
        installdir: String,
    },
}

impl GameHaven {
    pub(crate) fn library_page(&mut self, ui: &mut MaterialUi<'_>) {
        // 顶部工具行：左侧库数量，右侧刷新（M3 里操作放在行尾）。
        let count = self.libraries.len();
        let mut refresh = false;
        ui.horizontal(|ui| {
            widgets::caption(ui, &format!("{count} 个库"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                refresh = widgets::tonal_button_icon(
                    ui,
                    "library-refresh",
                    "刷新",
                    icons::ICON_REFRESH,
                    true,
                );
            });
        });
        ui.add_space(design::SPACE_4);

        // 注入集合每帧读一次：config.yaml 很小，不做额外缓存，点「刷新」后再读就是最新状态。
        let injected: std::collections::HashSet<u32> = steam::env::sls_config_path()
            .map(|path| sls::config::Config::new(path).apps().into_iter().collect())
            .unwrap_or_default();

        let ctx = ui.ctx().clone();
        // 先克隆库列表：遍历时不借用 self.libraries，封面缓存才能同时按需请求。
        let libraries = self.libraries.clone();
        // 行内操作先收集，遍历结束、借用释放后再改 self。
        let mut actions: Vec<RowAction> = Vec::new();
        if libraries.is_empty() {
            widgets::empty_state(
                ui,
                icons::ICON_STORAGE,
                "没有发现 Steam 库",
                "请先启动 Steam 并完成初始化，再回到这里点「刷新」重新扫描。",
            );
        } else {
            for (index, library) in libraries.iter().enumerate() {
                actions.extend(library_card(
                    ui,
                    index,
                    library,
                    &injected,
                    &mut self.covers,
                ));
                ui.add_space(design::SPACE_3);
            }
        }

        // 移除写 config.yaml，启动走 steam://rungameid；两者都只改状态，结果在同一帧显示在列表下方。
        for action in actions {
            match action {
                RowAction::Remove(app_id) => self.remove_from_slssteam(app_id, &ctx),
                RowAction::Launch { library, app_id } => {
                    if let Some(library) = libraries.get(library) {
                        self.launch_game(library, app_id);
                    }
                }
                RowAction::Repair {
                    library,
                    app_id,
                    installdir,
                } => {
                    if let Some(library) = libraries.get(library) {
                        self.start_repair(library, app_id, &installdir, &ctx);
                    }
                }
            }
        }

        // 操作结果放在列表之后、说明行之前，不遮挡游戏列表。
        if let Some(result) = &self.remove_result {
            widgets::result_banner(ui, result);
        }
        if let Some(result) = &self.launch_result {
            widgets::result_banner(ui, result);
        }
        if let crate::jobs::RepairJob::Done(result) = &self.repair {
            widgets::result_banner(ui, result);
        }

        // 底部说明：游戏卡片与 DLC 信息留到下一阶段。
        ui.add_space(design::SPACE_3);
        widgets::caption(
            ui,
            "游戏卡片与 DLC 信息放到下一阶段，当前先保证添加、启动与修复链路可用。",
        );
        ui.add_space(design::SPACE_3);
    }
}

/// 行内固定列的横向几何：所有行共用，保证状态 / 占用 / 操作逐列对齐。
struct Columns {
    cover: Rect,
    name: Rect,
    chip: Rect,
    meta: Rect,
    actions: Rect,
}

fn columns(origin: f32, width: f32, y: egui::Rangef) -> Columns {
    let pad = design::SPACE_2;
    let left = origin + pad;
    let right = origin + width - pad;
    // 窄窗口逐列降级：先丢占用，再丢状态，名称与操作始终保留。
    let show_meta = width >= 520.0;
    let show_chip = width >= 380.0;
    let cover = Rect::from_x_y_ranges(left..=left + covers::COVER_SIZE.x, y);
    let actions = Rect::from_x_y_ranges(right - design::COL_ACTIONS..=right, y);
    let meta_anchor = actions.left() - design::COL_GAP;
    let meta = if show_meta {
        Rect::from_x_y_ranges(meta_anchor - design::COL_META..=meta_anchor, y)
    } else {
        Rect::from_x_y_ranges(meta_anchor..=meta_anchor, y)
    };
    let chip_anchor = meta.left() - design::COL_GAP;
    let chip = if show_chip {
        Rect::from_x_y_ranges(chip_anchor - design::COL_CHIP..=chip_anchor, y)
    } else {
        Rect::from_x_y_ranges(chip_anchor..=chip_anchor, y)
    };
    let name_right = if show_chip {
        chip.left()
    } else if show_meta {
        meta.left()
    } else {
        actions.left()
    } - design::COL_GAP;
    let name = Rect::from_x_y_ranges(
        cover.right() + design::COL_GAP..=(name_right.max(cover.right() + design::COL_GAP + 24.0)),
        y,
    );
    Columns {
        cover,
        name,
        chip,
        meta,
        actions,
    }
}

/// 单个库的卡片：客户端类型、安装记录数、库路径与游戏列表。
///
/// 返回值为各游戏行产生的操作，交给页面层统一执行；`library_index` 是库在列表里的位置。
fn library_card(
    ui: &mut MaterialUi<'_>,
    library_index: usize,
    library: &steam_library::Library,
    injected: &std::collections::HashSet<u32>,
    covers: &mut covers::Covers,
) -> Vec<RowAction> {
    // 每次重建卡片都重读 appmanifest，点「刷新」后列表就是最新的。
    let apps = steam_library::installed_apps(library);
    let count = apps.len();
    let client = library.client.name();
    let path = library.path.display().to_string();
    let width = ui.available_width();
    let mut actions = Vec::new();
    let accent = design::primary(ui);
    let variant = design::on_surface_variant(ui);
    let client_icon = if library.client == steam_library::Client::Flatpak {
        icons::ICON_PACKAGE_2
    } else {
        icons::ICON_DESKTOP_WINDOWS
    };

    Card::new(format!("library-{path}"), format!("{client} · {path}"))
        .variant(CardVariant::Filled)
        .width(egui::Rangef::new(width, width))
        .content_padding(Card::CONTENT_PADDING)
        .show(ui, |ui| {
            // 表头：客户端图标 + 名称靠左，安装记录数靠右。
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                widgets::paint_icon(
                    ui,
                    rect.center(),
                    Align2::CENTER_CENTER,
                    client_icon,
                    20.0,
                    accent,
                    true,
                );
                ui.add_space(design::SPACE_2);
                ui.label(RichText::new(client).size(design::TEXT_TITLE).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::status_chip(ui, &format!("{count} 个安装记录"), accent);
                });
            });
            ui.add_space(design::SPACE_1);
            let response = ui.add(
                egui::Label::new(
                    RichText::new(&path)
                        .font(design::mono(design::TEXT_LABEL))
                        .color(variant),
                )
                .truncate(),
            );
            response.on_hover_text(&path);

            ui.add_space(design::SPACE_3);
            if apps.is_empty() {
                widgets::caption(ui, "这个库还没有 appmanifest 记录");
            } else {
                // 列头：与游戏行同一套几何，让对齐关系一眼可见。
                let (head, _) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());
                let cols = columns(head.left(), width, head.y_range());
                let head_font = FontId::proportional(design::TEXT_LABEL);
                let painter = ui.painter();
                painter.text(cols.name.left_center(), Align2::LEFT_CENTER, "游戏", head_font.clone(), variant);
                if cols.chip.width() > 0.0 {
                    painter.text(cols.chip.right_center(), Align2::RIGHT_CENTER, "状态", head_font.clone(), variant);
                }
                if cols.meta.width() > 0.0 {
                    painter.text(cols.meta.right_center(), Align2::RIGHT_CENTER, "占用", head_font.clone(), variant);
                }
                painter.text(cols.actions.right_center(), Align2::RIGHT_CENTER, "操作", head_font, variant);
                ui.add_space(design::SPACE_1);
                for (index, app) in apps.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(2.0);
                    }
                    let is_injected = injected.contains(&app.app_id);
                    if let Some(action) = game_row(ui, library_index, app, is_injected, covers) {
                        actions.push(action);
                    }
                }
            }
        });

    actions
}

/// 单行游戏：固定五列网格，悬停整行叠层；返回值是二次确认后要求移除或启动的动作。
fn game_row(
    ui: &mut MaterialUi<'_>,
    library_index: usize,
    app: &steam_library::InstalledApp,
    injected: bool,
    covers: &mut covers::Covers,
) -> Option<RowAction> {
    let name = if app.name.is_empty() {
        format!("App {}", app.app_id)
    } else {
        app.name.clone()
    };
    let initial = name.chars().next().unwrap_or('?');
    let meta = format!("#{} · {}", app.app_id, format_size(app.size_on_disk));
    let incomplete = app.size_on_disk == 0;


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

    let width = ui.available_width();
    let (rect, hover) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
    if hover.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(vec2(0.0, 1.0)),
            CornerRadius::same(design::RADIUS_SM),
            tint(design::on_surface(ui), ui.tokens().state.hover),
        );
    }
    let cols = columns(rect.left(), width, rect.y_range());
    let rt = ui.rt();
    let mut action = None;

    // 封面列：纹理就绪就贴图，否则画同尺寸占位块，避免纹理到位时列表跳动。
    let cover_rect = Rect::from_center_size(cols.cover.center(), covers::COVER_SIZE);
    cover(ui, covers, app.app_id, initial, cover_rect);

    // 名称列：占剩余宽度，过长时截断，悬停可看到完整名称。
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(cols.name)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            let response = ui.add(
                egui::Label::new(RichText::new(name.as_str()).size(design::TEXT_BODY)).truncate(),
            );
            response.on_hover_text(&name);
        },
    );

    // 状态列：固定槽位；已注入可点击移除，未完成给 WARN chip，其余留空不漂移。
    if (injected || incomplete) && cols.chip.width() > 0.0 {
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(cols.chip)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                let mut mui = rt.ui(ui);
                if injected {
                    let color = design::primary(&mui);
                    if widgets::action_chip(&mut mui, "已注入", color) {
                        confirm = true;
                    }
                } else {
                    widgets::status_chip(&mut mui, "未完成", design::WARN);
                }
            },
        );
    }

    // 占用列：等宽数字右对齐；未完成记录整列 WARN 色。
    if cols.meta.width() > 0.0 {
        ui.painter().text(
        cols.meta.right_center(),
        Align2::RIGHT_CENTER,
        meta,
        design::mono(design::TEXT_LABEL),
            if incomplete { design::WARN } else { design::on_surface_variant(ui) },
        );
    }

    // 操作列：启动始终贴右，修复只在未完成时出现在它左边。
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(cols.actions)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            let mut mui = rt.ui(ui);
            mui.spacing_mut().item_spacing.x = design::SPACE_1;
            if widgets::icon_button(
                &mut mui,
                &format!("launch-{}", app.app_id),
                icons::ICON_PLAY_ARROW,
                "启动",
                IconButtonVariant::Tonal,
                true,
            ) {
                action = Some(RowAction::Launch {
                    library: library_index,
                    app_id: app.app_id,
                });
            }
            if incomplete
                && widgets::icon_button(
                    &mut mui,
                    &format!("repair-{}", app.app_id),
                    icons::ICON_BUILD_CIRCLE,
                    "修复 ACF",
                    IconButtonVariant::Standard,
                    true,
                )
            {
                action = Some(RowAction::Repair {
                    library: library_index,
                    app_id: app.app_id,
                    installdir: app.installdir.clone(),
                });
            }
        },
    );

    // 二次确认：只有上一帧点过「移除」才展开，取消与确认都回到未确认状态。
    if injected && show_confirm {
        ui.add_space(design::SPACE_1);
        ui.horizontal(|ui| {
            ui.add_space(cols.name.left() - rect.left());
            egui::Frame::new()
                .fill(tint(design::on_surface(ui), 0.05))
                .corner_radius(CornerRadius::same(design::RADIUS_MD))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("确认从 SLSsteam 配置移除？")
                            .size(design::TEXT_BODY),
                    );
                    ui.add_space(design::SPACE_2);
                    ui.horizontal(|ui| {
                        let mut mui = rt.ui(ui);
                        if widgets::filled_button(
                            &mut mui,
                            &format!("remove-confirm-{}", app.app_id),
                            "确认移除",
                            true,
                        ) {
                            confirm = false;
                            action = Some(RowAction::Remove(app.app_id));
                        }
                        mui.add_space(design::SPACE_2);
                        if widgets::text_button(
                            &mut mui,
                            &format!("remove-cancel-{}", app.app_id),
                            "取消",
                        ) {
                            confirm = false;
                        }
                    });
                });
        });
    }

    ctx.data_mut(|data| {
        *data.get_temp_mut_or_default::<bool>(confirm_id) = confirm;
    });

    action
}

/// 行首封面：纹理就绪就贴圆角图，否则画同尺寸占位块，中间放名称首字符。
fn cover(
    ui: &mut MaterialUi<'_>,
    covers: &mut covers::Covers,
    app_id: u32,
    initial: char,
    rect: Rect,
) {
    // 已请求过（成功、失败或下载中）时直接复用缓存，重复调用只查一次表。
    covers.request(app_id);
    if let Some(texture) = covers.texture(app_id) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.add(
                egui::Image::from_texture(texture)
                    .fit_to_exact_size(covers::COVER_SIZE)
                    .corner_radius(CornerRadius::same(6)),
            );
        });
        return;
    }

    let variant = design::on_surface_variant(ui);
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(6),
        tint(variant, 0.12),
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(6),
        egui::Stroke::new(1.0, design::outline_variant(ui)),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initial,
        FontId::proportional(design::TEXT_TITLE),
        variant,
    );
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