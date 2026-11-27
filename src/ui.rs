//! 界面骨架与共享组件：侧栏入口、页头、分组卡片和状态色。

use eframe::egui::{self, Color32, RichText};
use egui_mineral_material::MaterialUi;
use egui_mineral_material::prelude::{Card, CardVariant};

pub const SIDEBAR_WIDTH: f32 = 184.0;

pub const OK: Color32 = Color32::from_rgb(0x3f, 0xa9, 0x5f);
pub const WARN: Color32 = Color32::from_rgb(0xd9, 0x9a, 0x2b);

/// 侧栏导航项，宽度占满侧栏。
pub fn nav_item(ui: &mut MaterialUi<'_>, label: &str, selected: bool) -> bool {
    let width = ui.available_width();
    ui.add_sized(
        [width, 32.0],
        egui::Button::new(RichText::new(label).size(14.0)).selected(selected),
    )
    .clicked()
}

/// 页面标题区：大标题 + 一行说明 + 分隔线。
pub fn page_header(ui: &mut MaterialUi<'_>, title: &str, subtitle: &str) {
    let weak = ui.visuals().weak_text_color();
    ui.label(RichText::new(title).size(21.0).strong());
    ui.add_space(3.0);
    ui.label(RichText::new(subtitle).size(12.5).color(weak));
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(12.0);
}

/// 一个带标题的分组卡片。
pub fn section(ui: &mut MaterialUi<'_>, title: &str, add: impl FnOnce(&mut MaterialUi<'_>)) {
    ui.label(RichText::new(title).size(14.5).strong());
    ui.add_space(6.0);
    let width = ui.available_width();
    Card::new(title.to_owned(), title.to_owned())
        .variant(CardVariant::Filled)
        .width(egui::Rangef::new(width, width))
        .content_padding(Card::CONTENT_PADDING)
        .show(ui, |ui| add(ui));
    ui.add_space(14.0);
}

pub fn status_dot(ui: &mut MaterialUi<'_>, color: Color32) {
    ui.label(RichText::new("●").color(color));
}

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

/// 路径选择行：按钮 + 当前值 + 清除。返回是否点击了选择按钮。
pub fn path_input(ui: &mut MaterialUi<'_>, label: &str, action: &str, value: &mut Option<std::path::PathBuf>) -> bool {
    ui.label(label);
    ui.horizontal(|ui| {
        let browse = ui.button(action).clicked();
        if value.is_some() && ui.button("清除").on_hover_text(format!("清除{label}")).clicked() {
            *value = None;
        }
        if let Some(path) = value {
            let display_path = path.to_string_lossy();
            ui.add(egui::Label::new(display_path.as_ref()).truncate())
                .on_hover_text(display_path.as_ref());
        } else {
            ui.weak("尚未选择");
        }
        browse
    })
    .inner
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
