//! 组件封装：页面只调用这里的函数，保证全站同一套对齐与配色语言。

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke,
    vec2,
};
use egui_mineral_material::prelude::{
    ButtonVariant, Card, CardVariant, IconButton, IconButtonVariant, MaterialExt, MaterialIcon,
    PushButton, Size, Switch,
};
use egui_mineral_material::{MaterialUi, icons};

use crate::design::*;

/// 在指定位置画一枚 Material Symbols 图标。
pub fn paint_icon(
    ui: &egui::Ui,
    pos: Pos2,
    align: Align2,
    icon: MaterialIcon,
    size: f32,
    color: Color32,
    filled: bool,
) {
    ui.painter().text(
        pos,
        align,
        icon.codepoint(),
        FontId::new(size, icon_family(filled)),
        color,
    );
}

/// 横向渐变圆角块：品牌标记等需要跨色相的地方用。
pub fn gradient_rect(ui: &egui::Ui, rect: Rect, radius: u8, from: Color32, to: Color32) {
    let mut mesh = egui::epaint::Mesh::default();
    let shape = egui::Shape::Rect(egui::epaint::RectShape::filled(
        rect,
        CornerRadius::same(radius),
        Color32::WHITE,
    ));
    let mut tessellator = egui::epaint::tessellator::Tessellator::new(
        ui.pixels_per_point(),
        Default::default(),
        [1, 1],
        Vec::new(),
    );
    tessellator.tessellate_shape(shape, &mut mesh);
    for vertex in &mut mesh.vertices {
        let t = ((vertex.pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
        vertex.color = Color32::from_rgb(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()));
    }
    ui.painter().add(mesh);
}

/// 卡片分组：图标 + 标题在卡片内做表头，细分隔线之下才是内容。
pub fn section(
    ui: &mut MaterialUi<'_>,
    icon: MaterialIcon,
    title: &str,
    add: impl FnOnce(&mut MaterialUi<'_>),
) {
    section_inner(ui, Some(icon), None, title, add);
}

/// 带步骤编号的卡片分组（添加页的四步）。
pub fn section_step(
    ui: &mut MaterialUi<'_>,
    step: u32,
    title: &str,
    add: impl FnOnce(&mut MaterialUi<'_>),
) {
    section_inner(ui, None, Some(step), title, add);
}

fn section_inner(
    ui: &mut MaterialUi<'_>,
    icon: Option<MaterialIcon>,
    step: Option<u32>,
    title: &str,
    add: impl FnOnce(&mut MaterialUi<'_>),
) {
    let width = ui.available_width();
    let line = outline_variant(ui);
    let accent = primary(ui);
    Card::new(title.to_owned(), title.to_owned())
        .variant(CardVariant::Filled)
        .width(egui::Rangef::new(width, width))
        .content_padding(Card::CONTENT_PADDING)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(step) = step {
                    let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 11.0, tint(accent, 0.18));
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        step.to_string(),
                        FontId::proportional(TEXT_LABEL),
                        accent,
                    );
                } else if let Some(icon) = icon {
                    let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                    paint_icon(ui, rect.center(), Align2::CENTER_CENTER, icon, 20.0, accent, true);
                }
                ui.add_space(SPACE_1);
                ui.label(RichText::new(title).size(TEXT_TITLE).strong());
            });
            ui.add_space(SPACE_2);
            let y = ui.cursor().top();
            let left = ui.cursor().left();
            ui.painter().line_segment(
                [egui::pos2(left, y), egui::pos2(left + ui.available_width(), y)],
                Stroke::new(1.0, line),
            );
            ui.add_space(SPACE_3);
            add(ui);
        });
    ui.add_space(SPACE_4);
}

/// 卡片内的次级小标题。
pub fn subhead(ui: &mut MaterialUi<'_>, icon: MaterialIcon, text: &str) {
    let variant = on_surface_variant(ui);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        paint_icon(ui, rect.center(), Align2::CENTER_CENTER, icon, 16.0, variant, true);
        ui.add_space(SPACE_1);
        ui.label(RichText::new(text).size(TEXT_BODY).strong().color(variant));
    });
}

fn button(
    ui: &mut MaterialUi<'_>,
    id: &str,
    label: &str,
    icon: Option<MaterialIcon>,
    variant: ButtonVariant,
    size: Size,
    enabled: bool,
) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        let mut push = PushButton::new(id.to_owned(), label)
            .variant(variant)
            .size(size);
        if let Some(icon) = icon {
            push = push.icon(icon);
        }
        ui.mineral(push).response().clicked()
    })
    .inner
}

/// 主操作按钮（Filled）。
pub fn filled_button(ui: &mut MaterialUi<'_>, id: &str, label: &str, enabled: bool) -> bool {
    button(ui, id, label, None, ButtonVariant::Filled, Size::Small, enabled)
}

pub fn filled_button_icon(
    ui: &mut MaterialUi<'_>,
    id: &str,
    label: &str,
    icon: MaterialIcon,
    enabled: bool,
) -> bool {
    button(ui, id, label, Some(icon), ButtonVariant::Filled, Size::Small, enabled)
}

/// 次操作按钮（Tonal）。
pub fn tonal_button(ui: &mut MaterialUi<'_>, id: &str, label: &str, enabled: bool) -> bool {
    button(ui, id, label, None, ButtonVariant::Tonal, Size::Small, enabled)
}

pub fn tonal_button_icon(
    ui: &mut MaterialUi<'_>,
    id: &str,
    label: &str,
    icon: MaterialIcon,
    enabled: bool,
) -> bool {
    button(ui, id, label, Some(icon), ButtonVariant::Tonal, Size::Small, enabled)
}

/// 轻操作按钮（Text）。
pub fn text_button(ui: &mut MaterialUi<'_>, id: &str, label: &str) -> bool {
    button(ui, id, label, None, ButtonVariant::Text, Size::Small, true)
}

/// 行内小按钮（XSmall），用于游戏行与账号行尾。
pub fn row_button(
    ui: &mut MaterialUi<'_>,
    id: &str,
    label: &str,
    icon: Option<MaterialIcon>,
    variant: ButtonVariant,
    enabled: bool,
) -> bool {
    button(ui, id, label, icon, variant, Size::XSmall, enabled)
}

/// 行内图标按钮（M3 40dp）：悬停提示承担文字标签。
pub fn icon_button(
    ui: &mut MaterialUi<'_>,
    id: &str,
    icon: MaterialIcon,
    tooltip: &str,
    variant: IconButtonVariant,
    enabled: bool,
) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        let response = ui
            .mineral(
                IconButton::new(id.to_owned(), icon, tooltip.to_owned())
                    .variant(variant)
                    .size(Size::Small),
            )
            .response()
            .clone();
        let clicked = response.clicked();
        response.on_hover_text(tooltip);
        clicked
    })
    .inner
}

/// M3 双行列表行：左标题 + 说明，右控制区垂直居中；任何宽度都不折行。
pub fn control_row<R>(
    ui: &mut MaterialUi<'_>,
    label: &str,
    supporting: Option<&str>,
    control: impl FnOnce(&mut MaterialUi<'_>) -> R,
) -> R {
    let rt = ui.rt();
    let width = ui.available_width();
    let height = if supporting.is_some() { 72.0 } else { 56.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let surface = on_surface(ui);
    let variant = on_surface_variant(ui);
    let wrap = (width - 232.0).max(120.0);
    let label_g = ui.painter().layout(
        label.to_owned(),
        FontId::proportional(TEXT_BODY + 2.0),
        surface,
        wrap,
    );
    let sup_g = supporting.map(|text| {
        ui.painter()
            .layout(text.to_owned(), FontId::proportional(TEXT_BODY), variant, wrap)
    });
    let text_h = label_g.size().y + sup_g.as_ref().map_or(0.0, |g| g.size().y + 2.0);
    let mut y = rect.center().y - text_h / 2.0;
    ui.painter()
        .galley(egui::pos2(rect.left(), y), label_g, surface);
    if let Some(g) = sup_g {
        y += g.size().y + 2.0;
        ui.painter().galley(egui::pos2(rect.left(), y), g, variant);
    }
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| control(&mut rt.ui(ui)),
    )
    .inner
}

/// 设置行：左标题 + 说明，右开关，整行垂直居中。
pub fn switch_row(
    ui: &mut MaterialUi<'_>,
    id: &str,
    label: &str,
    supporting: &str,
    value: &mut bool,
) -> bool {
    control_row(ui, label, Some(supporting), |ui| {
        ui.mineral(Switch::new(id.to_owned(), label, value).label_visible(false))
            .changed()
    })
}

/// 状态 pill：语义色的半透明叠层底。
pub fn status_chip(ui: &mut MaterialUi<'_>, text: &str, color: Color32) {
    chip(ui, text, color, Sense::hover());
}

/// 可点击的状态 chip，用于行内轻量操作（例如「已注入」→ 点击进入移除确认）。
pub fn action_chip(ui: &mut MaterialUi<'_>, text: &str, color: Color32) -> bool {
    let (_rect, response) = chip(ui, text, color, Sense::click());
    let clicked = response.clicked();
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}

fn chip(ui: &mut MaterialUi<'_>, text: &str, color: Color32, sense: Sense) -> (Rect, egui::Response) {
    let font = FontId::proportional(TEXT_BODY);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = vec2(galley.size().x + 32.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let background = if response.hovered() && sense == Sense::click() {
        tint(color, 0.30)
    } else {
        tint(color, 0.16)
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_SM), background);
    let position = rect.center() - galley.size() / 2.0;
    ui.painter().galley(position, galley, color);
    (rect, response)
}

/// 侧栏导航项：图标 + 标签，选中为 pill。
pub fn nav_item(ui: &mut MaterialUi<'_>, icon: MaterialIcon, label: &str, selected: bool) -> bool {
    let tokens = ui.tokens();
    let color = if selected {
        tokens.color.on_secondary_container
    } else {
        tokens.color.on_surface_variant
    };
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, NAV_ITEM_HEIGHT), Sense::click());
    let pill = rect.shrink2(vec2(0.0, 2.0));
    if selected {
        ui.painter()
            .rect_filled(pill, CornerRadius::same(RADIUS_PILL), tokens.color.secondary_container);
    } else if response.hovered() {
        ui.painter().rect_filled(
            pill,
            CornerRadius::same(RADIUS_PILL),
            tint(tokens.color.on_surface, tokens.state.hover),
        );
    }
    paint_icon(
        ui,
        egui::pos2(pill.left() + SPACE_4, pill.center().y),
        Align2::LEFT_CENTER,
        icon,
        24.0,
        color,
        selected,
    );
    ui.painter().text(
        egui::pos2(pill.left() + SPACE_4 + 36.0, pill.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(TEXT_BODY),
        color,
    );
    let clicked = response.clicked();
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}

pub fn empty_state(ui: &mut MaterialUi<'_>, icon: MaterialIcon, title: &str, body: &str) {
    let variant = on_surface_variant(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(SPACE_6);
        let (rect, _) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::hover());
        paint_icon(ui, rect.center(), Align2::CENTER_CENTER, icon, 48.0, tint(variant, 0.55), false);
        ui.add_space(SPACE_3);
        ui.label(RichText::new(title).size(TEXT_TITLE).strong());
        ui.add_space(SPACE_2);
        ui.label(RichText::new(body).size(TEXT_BODY).color(variant));
        ui.add_space(SPACE_6);
    });
}

/// 左标签、右等宽值的列表行；值超宽截断，悬停看全。
pub fn key_value(ui: &mut MaterialUi<'_>, label: &str, value: &str) {
    let variant = on_surface_variant(ui);
    let surface = on_surface(ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(TEXT_BODY).color(variant));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let response = ui.add(
                egui::Label::new(RichText::new(value).font(mono(TEXT_LABEL)).color(surface))
                    .truncate(),
            );
            response.on_hover_text(value);
        });
    });
}

/// 路径行：缺失时给 WARN 色的明确提示，避免整行消失让布局跳动。
pub fn path_row(ui: &mut MaterialUi<'_>, label: &str, value: Option<&str>) {
    match value {
        Some(value) => key_value(ui, label, value),
        None => {
            let warn = WARN;
            let variant = on_surface_variant(ui);
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(TEXT_BODY).color(variant));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("未找到").size(TEXT_LABEL).color(warn));
                });
            });
        }
    }
}

pub fn caption(ui: &mut MaterialUi<'_>, text: &str) {
    let variant = on_surface_variant(ui);
    ui.label(RichText::new(text).size(TEXT_LABEL).color(variant));
}

pub fn error_text(ui: &mut MaterialUi<'_>, text: &str) {
    let color = error(ui);
    ui.label(RichText::new(text).size(TEXT_BODY).color(color));
}

/// 操作结果横幅：语义色叠层底 + 左侧色条 + 图标，成功绿 / 失败红。
pub fn result_banner(ui: &mut MaterialUi<'_>, result: &Result<String, String>) {
    let (color, icon) = match result {
        Ok(_) => (OK, icons::ICON_CHECK_CIRCLE),
        Err(_) => (error(ui), icons::ICON_ERROR),
    };
    let message = match result {
        Ok(message) | Err(message) => message.as_str(),
    };
    let surface = on_surface(ui);
    ui.add_space(SPACE_2);
    let response = egui::Frame::new()
        .fill(tint(color, 0.12))
        .corner_radius(CornerRadius::same(RADIUS_MD))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                paint_icon(ui, rect.center(), Align2::CENTER_CENTER, icon, 18.0, color, true);
                ui.add_space(SPACE_1);
                ui.label(RichText::new(message).size(TEXT_BODY).color(surface));
            });
        })
        .response;
    let bar = Rect::from_min_max(
        response.rect.left_top(),
        egui::pos2(response.rect.left() + 3.0, response.rect.bottom()),
    );
    ui.painter().rect_filled(
        bar,
        CornerRadius {
            nw: RADIUS_MD,
            sw: RADIUS_MD,
            ne: 0,
            se: 0,
        },
        color,
    );
}

/// 可点击的列表行（搜索结果）：左名称、右等宽 ID，悬停有叠层。
pub fn list_row(ui: &mut MaterialUi<'_>, id: &str, label: &str, trailing: &str) -> bool {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 48.0), Sense::hover());
    let response = ui.interact(rect, ui.scope_id().with(id), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(vec2(0.0, 1.0)),
            CornerRadius::same(RADIUS_SM),
            tint(on_surface(ui), ui.tokens().state.hover),
        );
    }
    let inner = rect.shrink2(vec2(SPACE_2, 0.0));
    let surface = on_surface(ui);
    let variant = on_surface_variant(ui);
    ui.painter().text(
        inner.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(TEXT_BODY),
        surface,
    );
    ui.painter().text(
        inner.right_center(),
        Align2::RIGHT_CENTER,
        trailing,
        mono(TEXT_LABEL),
        variant,
    );
    let clicked = response.clicked();
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}

/// 日志行：左等宽时间、右消息。
pub fn journal_row(ui: &mut MaterialUi<'_>, time: &str, message: &str) {
    let variant = on_surface_variant(ui);
    let surface = on_surface(ui);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(92.0, 20.0), Sense::hover());
        ui.painter().text(
            rect.left_center(),
            Align2::LEFT_CENTER,
            time,
            mono(TEXT_LABEL),
            variant,
        );
        ui.add(
            egui::Label::new(RichText::new(message).size(TEXT_BODY).color(surface))
                .truncate(),
        );
    });
}

/// 圆形头像块：首字符 + 主色叠层底。
pub fn avatar(ui: &mut MaterialUi<'_>, initial: char, size: f32) {
    let accent = primary(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), size / 2.0, tint(accent, 0.18));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initial.to_string(),
        FontId::proportional(size * 0.45),
        accent,
    );
}