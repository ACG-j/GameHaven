//! 基于 egui_mineral_material 的组件封装。页面只调用这里的函数。

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, RichText, Sense, vec2,
};
use egui_mineral_material::MaterialUi;
use egui_mineral_material::prelude::{
    ButtonVariant, Card, CardVariant, MaterialExt, PushButton, Switch,
};

use crate::design::*;

/// 带标题的卡片分组。
pub fn section(ui: &mut MaterialUi<'_>, title: &str, add: impl FnOnce(&mut MaterialUi<'_>)) {
    ui.label(RichText::new(title).size(TEXT_TITLE).strong());
    ui.add_space(SPACE_2);
    let width = ui.available_width();
    Card::new(title.to_owned(), title.to_owned())
        .variant(CardVariant::Filled)
        .width(egui::Rangef::new(width, width))
        .content_padding(Card::CONTENT_PADDING)
        .show(ui, |ui| add(ui));
    ui.add_space(SPACE_5);
}

/// 主操作按钮（Filled）。
pub fn filled_button(ui: &mut MaterialUi<'_>, id: &str, label: &str, enabled: bool) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        ui.mineral(PushButton::new(id.to_owned(), label).variant(ButtonVariant::Filled))
            .response()
            .clicked()
    })
    .inner
}

/// 次操作按钮（Tonal）。
pub fn tonal_button(ui: &mut MaterialUi<'_>, id: &str, label: &str, enabled: bool) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        ui.mineral(PushButton::new(id.to_owned(), label).variant(ButtonVariant::Tonal))
            .response()
            .clicked()
    })
    .inner
}

/// 轻操作按钮（Text）。
pub fn text_button(ui: &mut MaterialUi<'_>, id: &str, label: &str) -> bool {
    ui.mineral(PushButton::new(id.to_owned(), label).variant(ButtonVariant::Text))
        .response()
        .clicked()
}

/// M3 开关，返回是否改变。
pub fn switch(ui: &mut MaterialUi<'_>, id: &str, label: &str, value: &mut bool) -> bool {
    ui.mineral(Switch::new(id.to_owned(), label, value)).changed()
}

/// 状态 pill：底色是语义色的 18% 透明层。
pub fn status_chip(ui: &mut MaterialUi<'_>, text: &str, color: Color32) {
    let font = FontId::proportional(TEXT_LABEL);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, color);
    let padding = vec2(SPACE_2 + 2.0, 3.0);
    let size = galley.size() + padding * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_SM), color.gamma_multiply(0.18));
    let position = rect.center() - galley.size() / 2.0;
    ui.painter().galley(position, galley, color);
}

/// 可点击的状态 chip，用于行内轻量操作（例如「已注入」→ 点击进入移除确认）。
pub fn action_chip(ui: &mut MaterialUi<'_>, text: &str, color: Color32) -> bool {
    let font = FontId::proportional(TEXT_LABEL);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let padding = vec2(SPACE_2 + 2.0, 3.0);
    let size = galley.size() + padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let background = if response.hovered() {
        color.gamma_multiply(0.30)
    } else {
        color.gamma_multiply(0.18)
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS_SM), background);
    let position = rect.center() - galley.size() / 2.0;
    ui.painter().galley(position, galley, color);
    let clicked = response.clicked();
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    clicked
}

/// M3 NavigationDrawer 项：56dp 高，选中为 pill。
pub fn nav_item(ui: &mut MaterialUi<'_>, label: &str, selected: bool) -> bool {
    let tokens = ui.tokens();
    let color = if selected {
        tokens.color.on_secondary_container
    } else {
        tokens.color.on_surface_variant
    };
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, NAV_ITEM_HEIGHT), Sense::click());
    let pill = rect.shrink(2.0);
    if selected {
        ui.painter()
            .rect_filled(pill, CornerRadius::same(RADIUS_PILL), tokens.color.secondary_container);
    } else if response.hovered() {
        ui.painter().rect_filled(
            pill,
            CornerRadius::same(RADIUS_PILL),
            tokens.color.on_surface.gamma_multiply(0.06),
        );
    }
    ui.painter().text(
        rect.left_center() + vec2(SPACE_5, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(TEXT_BODY),
        color,
    );
    response.clicked()
}

pub fn empty_state(ui: &mut MaterialUi<'_>, title: &str, body: &str) {
    let variant = on_surface_variant(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(SPACE_6);
        ui.label(RichText::new(title).size(TEXT_TITLE).strong());
        ui.add_space(SPACE_2);
        ui.label(RichText::new(body).size(TEXT_BODY).color(variant));
        ui.add_space(SPACE_6);
    });
}

/// 左标签、右值的列表行。
pub fn key_value(ui: &mut MaterialUi<'_>, label: &str, value: &str) {
    let variant = on_surface_variant(ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(TEXT_BODY));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(TEXT_BODY).color(variant));
        });
    });
}

pub fn caption(ui: &mut MaterialUi<'_>, text: &str) {
    let variant = on_surface_variant(ui);
    ui.label(RichText::new(text).size(TEXT_LABEL).color(variant));
}

pub fn error_text(ui: &mut MaterialUi<'_>, text: &str) {
    let color = error(ui);
    ui.label(RichText::new(text).size(TEXT_BODY).color(color));
}
