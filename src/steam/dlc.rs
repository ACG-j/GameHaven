//! DLC 内容下载：对比 ACF 的 InstalledDepots，找出缺少的 DLC depot。

use std::path::Path;

use keyvalues_parser::{Obj, Value};

use crate::steam::appinfo::{self, DlcDepot};

/// 读取 ACF 里已登记的 depot ID。
pub fn installed_depots(acf: &Path) -> Vec<u32> {
    let Ok(text) = std::fs::read_to_string(acf) else {
        return Vec::new();
    };
    let Ok(vdf) = keyvalues_parser::parse(text.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    let value: Value<'static> = vdf.into_vdf().into_owned().value;
    let Some(app) = value.get_obj() else {
        return Vec::new();
    };
    let Some(depots) = field_obj(app, "InstalledDepots") else {
        return Vec::new();
    };
    depots
        .iter()
        .filter_map(|(key, _)| key.parse().ok())
        .collect()
}

fn field_obj<'a>(object: &'a Obj<'a>, name: &str) -> Option<&'a Obj<'a>> {
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))?
        .1
        .first()?
        .get_obj()
}

/// 游戏有、但 ACF 里没登记的 DLC depot。
pub fn missing(game_app_id: u32, acf: &Path) -> Result<Vec<DlcDepot>, String> {
    let info = appinfo::fetch(game_app_id)?;
    let installed = installed_depots(acf);
    Ok(info
        .dlc_depots
        .into_iter()
        .filter(|depot| !installed.contains(&depot.depot_id))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_installed_depot_ids() {
        let directory = tempfile::tempdir().unwrap();
        let acf = directory.path().join("appmanifest_394360.acf");
        std::fs::write(
            &acf,
            r#""AppState"
{
    "appid" "394360"
    "InstalledDepots"
    {
        "394361" { "manifest" "1" }
        "460559" { "manifest" "2" }
    }
}"#,
        )
        .unwrap();
        assert_eq!(installed_depots(&acf), vec![394361, 460559]);
    }

    #[test]
    fn missing_acf_returns_empty() {
        let directory = tempfile::tempdir().unwrap();
        assert!(installed_depots(&directory.path().join("nope.acf")).is_empty());
    }
}
