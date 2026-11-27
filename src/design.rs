#![allow(dead_code)] // token 集按 M3 完整保留，页面按需取用。

//! Material 3 设计 token：间距、圆角、字号与语义色。
//!
//! 页面和组件只从这里取值，避免再出现 2/6/10/14/20 这类魔法数。

use eframe::egui::Color32;
use egui_mineral_material::MaterialUi;
use egui_mineral_material::prelude::MaterialExt;

// 4dp 基准网格。
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;
pub const SPACE_6: f32 = 32.0;

// 布局。
pub const SIDEBAR_WIDTH: f32 = 240.0;
pub const NAV_ITEM_HEIGHT: f32 = 56.0;
pub const CONTENT_MAX_WIDTH: f32 = 880.0;
pub const WINDOW_MIN_WIDTH: f32 = 1024.0;
pub const WINDOW_MIN_HEIGHT: f32 = 700.0;

// 圆角。
pub const RADIUS_SM: u8 = 8;
pub const RADIUS_MD: u8 = 12;
pub const RADIUS_LG: u8 = 16;
pub const RADIUS_PILL: u8 = 28;

// M3 type scale 中实际用到的几档。
pub const TEXT_HEADLINE: f32 = 24.0;
pub const TEXT_TITLE: f32 = 16.0;
pub const TEXT_BODY: f32 = 14.0;
pub const TEXT_LABEL: f32 = 12.0;

/// 语义色：M3 没有 success / warning role，这里补两组，保证深色 surface 上的对比度。
pub const OK: Color32 = Color32::from_rgb(0x8f, 0xd5, 0xa6);
pub const WARN: Color32 = Color32::from_rgb(0xf2, 0xc9, 0x4c);

pub fn on_surface(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.on_surface
}

pub fn on_surface_variant(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.on_surface_variant
}

pub fn outline(ui: &MaterialUi<'_>) -> Color32 {
    ui.tokens().color.outline
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
