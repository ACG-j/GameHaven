use eframe::egui::{self, FontData, FontFamily};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use fontique::{Collection, FontStyle, FontWeight, FontWidth};

pub fn configure(ctx: &egui::Context) {
    let mut fonts = Collection::new(Default::default());
    for name in [
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Source Han Sans SC",
        "Source Han Sans CN",
        "Microsoft YaHei",
        "WenQuanYi Zen Hei",
    ] {
        let Some(family) = fonts.family_by_name(name) else {
            continue;
        };
        let Some(face) = family.match_font(
            FontWidth::NORMAL,
            FontStyle::Normal,
            FontWeight::NORMAL,
            false,
        ) else {
            continue;
        };
        let Some(blob) = face.load(None) else {
            continue;
        };
        let (bytes, _) = blob.into_raw_parts();
        let mut data = FontData::from_blob(bytes, face.index());
        // A variable font can have a default weight other than Regular.
        if data
            .variation_axes()
            .iter()
            .any(|axis| axis.tag.to_be_bytes() == *b"wght")
        {
            data.tweak.coords.push("wght", FontWeight::NORMAL.value());
        }
        ctx.add_font(FontInsert::new(
            "gamehaven-ui",
            data,
            vec![InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Highest,
            }],
        ));
        break;
    }
}
