#![allow(dead_code)] // token 集按 M3 完整保留，页面按需取用。

//! 设计 token：间距、圆角、字号、语义色与「深海夜航」配色。
//!
//! 页面和组件只从这里取值，避免再出现 2/6/10/14/20 这类魔法数。

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, Theme};
use egui_mineral_material::MaterialUi;
use egui_mineral_material::color::ColorScheme;
use egui_mineral_material::prelude::MaterialExt;
use egui_mineral_material::theme::Tokens;
use egui_mineral_material::Material;

// 4dp 基准网格。
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;
pub const SPACE_6: f32 = 32.0;

// 布局。
pub const SIDEBAR_WIDTH: f32 = 232.0;
pub const NAV_ITEM_HEIGHT: f32 = 56.0;
pub const CONTENT_MAX_WIDTH: f32 = 960.0;
pub const WINDOW_MIN_WIDTH: f32 = 1024.0;
pub const WINDOW_MIN_HEIGHT: f32 = 700.0;

// 游戏库行的固定列宽：封面 | 名称(弹性) | 状态 | 占用 | 操作。
pub const ROW_HEIGHT: f32 = 56.0;
pub const COL_GAP: f32 = 8.0;
pub const COL_CHIP: f32 = 76.0;
pub const COL_META: f32 = 140.0;
pub const COL_ACTIONS: f32 = 92.0;

// 圆角。
pub const RADIUS_SM: u8 = 8;
pub const RADIUS_MD: u8 = 12;
pub const RADIUS_LG: u8 = 16;
pub const RADIUS_PILL: u8 = 28;

// 字号。
pub const TEXT_HEADLINE: f32 = 22.0;
pub const TEXT_TITLE: f32 = 16.0;
pub const TEXT_BODY: f32 = 14.0;
pub const TEXT_LABEL: f32 = 12.0;

/// 路径、AppID 等等宽内容用的字体。
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Material Symbols 图标字体族（bundled-icons 注册的名字）。
pub fn icon_family(filled: bool) -> FontFamily {
    FontFamily::Name(
        if filled {
            "material-icons"
        } else {
            "material-icons-outlined"
        }
        .into(),
    )
}

/// 语义色：M3 没有 success / warning role，这里补两组，保证深色 surface 上的对比度。
pub const OK: Color32 = Color32::from_rgb(0x7a, 0xdb, 0x8f);
pub const WARN: Color32 = Color32::from_rgb(0xff, 0xc6, 0x5c);

/// 半透明叠层：把语义色铺在任意 surface 上做 tint。
pub fn tint(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (alpha * 255.0).round() as u8,
    )
}

/// 「深海夜航」暗色 scheme：墨蓝底 + 电蓝主色 + 琥珀 tertiary。
fn harbor_scheme() -> ColorScheme {
    let rgb = |r: u8, g: u8, b: u8| Color32::from_rgb(r, g, b);
    let mut c = ColorScheme::baseline(Theme::Dark);
    c.primary = rgb(0x4d, 0xc3, 0xff);
    c.on_primary = rgb(0x00, 0x27, 0x39);
    c.primary_container = rgb(0x0d, 0x3b, 0x55);
    c.on_primary_container = rgb(0xc8, 0xe9, 0xff);
    c.inverse_primary = rgb(0x00, 0x65, 0x8f);
    c.primary_fixed = rgb(0xc8, 0xe9, 0xff);
    c.primary_fixed_dim = rgb(0x4d, 0xc3, 0xff);
    c.on_primary_fixed = rgb(0x00, 0x27, 0x39);
    c.on_primary_fixed_variant = rgb(0x0d, 0x3b, 0x55);
    c.secondary = rgb(0x8b, 0xd3, 0xe6);
    c.on_secondary = rgb(0x00, 0x36, 0x42);
    c.secondary_container = rgb(0x12, 0x3a, 0x4c);
    c.on_secondary_container = rgb(0xc6, 0xe7, 0xf7);
    c.secondary_fixed = rgb(0xc6, 0xe7, 0xf7);
    c.secondary_fixed_dim = rgb(0x8b, 0xd3, 0xe6);
    c.on_secondary_fixed = rgb(0x00, 0x36, 0x42);
    c.on_secondary_fixed_variant = rgb(0x12, 0x3a, 0x4c);
    c.tertiary = rgb(0xff, 0xb4, 0x54);
    c.on_tertiary = rgb(0x45, 0x2b, 0x00);
    c.tertiary_container = rgb(0x5c, 0x3a, 0x00);
    c.on_tertiary_container = rgb(0xff, 0xdd, 0xb0);
    c.tertiary_fixed = rgb(0xff, 0xdd, 0xb0);
    c.tertiary_fixed_dim = rgb(0xff, 0xb4, 0x54);
    c.on_tertiary_fixed = rgb(0x45, 0x2b, 0x00);
    c.on_tertiary_fixed_variant = rgb(0x5c, 0x3a, 0x00);
    c.error = rgb(0xff, 0x7b, 0x72);
    c.on_error = rgb(0x45, 0x0a, 0x05);
    c.error_container = rgb(0x5c, 0x1a, 0x14);
    c.on_error_container = rgb(0xff, 0xda, 0xd6);
    c.surface_dim = rgb(0x0a, 0x0f, 0x16);
    c.surface = rgb(0x0d, 0x13, 0x1b);
    c.surface_tint = rgb(0x4d, 0xc3, 0xff);
    c.surface_bright = rgb(0x33, 0x42, 0x54);
    c.surface_container_lowest = rgb(0x0a, 0x0f, 0x16);
    c.surface_container_low = rgb(0x11, 0x19, 0x25);
    c.surface_container = rgb(0x18, 0x22, 0x2e);
    c.surface_container_high = rgb(0x1e, 0x2a, 0x38);
    c.surface_container_highest = rgb(0x26, 0x34, 0x44);
    c.on_surface = rgb(0xe9, 0xef, 0xf7);
    c.on_surface_variant = rgb(0x9d, 0xae, 0xc2);
    c.outline = rgb(0x4a, 0x5c, 0x70);
    c.outline_variant = rgb(0x2a, 0x39, 0x49);
    c.inverse_surface = rgb(0xe9, 0xef, 0xf7);
    c.inverse_on_surface = rgb(0x10, 0x17, 0x20);
    c.surface_variant = rgb(0x1e, 0x2a, 0x38);
    c.background = rgb(0x0d, 0x13, 0x1b);
    c.on_background = rgb(0xe9, 0xef, 0xf7);
    c.shadow = rgb(0x00, 0x00, 0x00);
    c.scrim = rgb(0x00, 0x00, 0x00);
    c.theme = Theme::Dark;
    c
}

/// 注入暗色 scheme 并锁定暗色偏好；浅色 slot 保留基线但不会被用到。
pub fn install_theme(ctx: &egui::Context, material: &Material) {
    ctx.options_mut(|options| options.theme_preference = egui::ThemePreference::Dark);
    material.set_tokens(ctx, Tokens::new(harbor_scheme()));
}

/// 每帧在 shell 里调用：输入井、滚动条与闲置描边跟随本套配色。
pub fn tune_visuals(ui: &mut MaterialUi<'_>) {
    let color = ui.tokens().color;
    let visuals = ui.visuals_mut();
    visuals.extreme_bg_color = color.surface_container_lowest;
    visuals.text_edit_bg_color = Some(color.surface_container_lowest);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, color.outline_variant);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, color.on_surface_variant);
    ui.ctx().style_mut_of(egui::Theme::Dark, |style| {
        style.spacing.scroll = egui::style::ScrollStyle {
            floating_width: 4.0,
            floating_allocated_width: 6.0,
            dormant_handle_opacity: 0.45,
            dormant_background_opacity: 0.0,
            active_handle_opacity: 0.75,
            interact_handle_opacity: 1.0,
            ..style.spacing.scroll
        };
    });
}

pub fn on_surface(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.on_surface
}

pub fn on_surface_variant(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.on_surface_variant
}

pub fn outline(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.outline
}

pub fn outline_variant(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.outline_variant
}

pub fn primary(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.primary
}

pub fn error(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.error
}

pub fn secondary_container(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.secondary_container
}

pub fn on_secondary_container(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.on_secondary_container
}

/// 校验 Steam AppID 输入；空输入返回空错误，界面据此隐藏提示。
pub fn validate_app_id(input: &str) -> Result<u32, &'static str> {
    let input = input.trim();
    if input.is_empty() {
        return Err("");
    }
    if input.bytes().any(|byte| !byte.is_ascii_digit()) {
        return Err("请输入有效的 Steam AppID（1–4294967295）。");
    }
    match input.parse::<u32>() {
        Ok(value) if value > 0 => Ok(value),
        _ => Err("请输入有效的 Steam AppID（1–4294967295）。"),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_app_id;

    #[test]
    fn accepts_positive_steam_app_ids() {
        assert_eq!(validate_app_id("440"), Ok(440));
        assert_eq!(validate_app_id(" 4294967295 "), Ok(u32::MAX));
    }

    #[test]
    fn rejects_empty_non_numeric_zero_and_overflow() {
        assert!(validate_app_id("").is_err());
        assert!(validate_app_id("abc").is_err());
        assert!(validate_app_id("0").is_err());
        assert!(validate_app_id("4294967296").is_err());
    }
}