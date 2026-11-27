use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, RichText};
use egui_mineral_material::MaterialUi;

use crate::design::{self, SPACE_1, SPACE_2, SPACE_3, TEXT_BODY, TEXT_LABEL};
use crate::steam::env::{self, Check, Level, Report};
use crate::widgets;

enum State {
    Idle,
    Pending(Receiver<Report>),
    Ready(Report),
}

/// 环境体检的后台任务与结果渲染。
pub struct EnvPanel {
    state: State,
}

impl EnvPanel {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut panel = Self { state: State::Idle };
        panel.refresh(ctx);
        panel
    }

    pub fn refresh(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("steam-env".into())
            .spawn(move || {
                let _ = sender.send(env::collect());
                ctx.request_repaint();
            }) {
            Ok(_) => self.state = State::Pending(receiver),
            Err(_) => self.state = State::Ready(Report::failure("无法启动环境检查线程。")),
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        let State::Pending(receiver) = &self.state else {
            return;
        };
        match receiver.try_recv() {
            Ok(report) => self.state = State::Ready(report),
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(120)),
            Err(TryRecvError::Disconnected) => {
                self.state = State::Ready(Report::failure("环境检查意外结束。"));
            }
        }
    }

    /// 返回 `(正常, 注意, 异常)`，尚未完成时返回 `None`。
    pub fn summary(&self) -> Option<(usize, usize, usize)> {
        match &self.state {
            State::Ready(report) => Some(report.counts()),
            _ => None,
        }
    }

    pub fn steam_running(&self) -> bool {
        match &self.state {
            State::Ready(report) => !report.processes.is_empty(),
            _ => false,
        }
    }

    /// 检查项列表：注意 / 异常直接展开，正常项收进折叠区。
    pub fn show_checks(&mut self, ui: &mut MaterialUi<'_>) {
        let State::Ready(report) = &self.state else {
            // 检查进行中的进度由页面顶部的汇总行显示，这里不重复。
            return;
        };

        if report.checks.is_empty() {
            widgets::empty_state(ui, "没有检查结果", "点「重新检查」重新读取 Steam 环境。");
            return;
        }

        for check in report
            .checks
            .iter()
            .filter(|check| check.level != Level::Ok)
        {
            show_attention(ui, check);
            ui.add_space(SPACE_3);
        }

        let ok_checks: Vec<&Check> = report
            .checks
            .iter()
            .filter(|check| check.level == Level::Ok)
            .collect();
        if !ok_checks.is_empty() {
            show_ok_group(ui, &ok_checks);
        }
    }
}

/// Check 等级对应的语义色。
fn level_color(level: Level, ui: &MaterialUi<'_>) -> egui::Color32 {
    match level {
        Level::Ok => design::OK,
        Level::Warn => design::WARN,
        Level::Error => design::error(ui),
    }
}

/// 注意 / 异常项：色点 + 标题 + 等级 pill + 详情 + 建议，直接展开。
fn show_attention(ui: &mut MaterialUi<'_>, check: &Check) {
    let color = level_color(check.level, ui);
    let variant = design::on_surface_variant(ui);
    ui.horizontal_top(|ui| {
        ui.label(RichText::new("●").size(TEXT_LABEL).color(color));
        ui.add_space(SPACE_2);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&check.title).size(TEXT_BODY).strong());
                ui.add_space(SPACE_2);
                widgets::status_chip(ui, check.level.label(), color);
            });
            ui.add_space(SPACE_1);
            ui.label(RichText::new(&check.detail).size(TEXT_BODY).color(variant));
            if let Some(hint) = &check.hint {
                ui.add_space(SPACE_1);
                ui.label(
                    RichText::new(format!("建议 · {hint}"))
                        .size(TEXT_LABEL)
                        .color(variant),
                );
            }
        });
    });
}

/// 正常项折叠区：标题只写数量，默认收起以减少页面噪音。
fn show_ok_group(ui: &mut MaterialUi<'_>, checks: &[&Check]) {
    let variant = design::on_surface_variant(ui);
    let header = RichText::new(format!("{} 项正常", checks.len()))
        .size(TEXT_BODY)
        .color(variant);
    // 折叠区的内容仍是 egui::Ui，需要重新包一层才能调用 Material 组件。
    let rt = ui.rt();
    let _ = egui::CollapsingHeader::new(header)
        .id_salt("env-ok-checks")
        .show_background(false)
        .show(ui, |body| {
            let mut mui = rt.ui(body);
            for (index, check) in checks.iter().enumerate() {
                if index > 0 {
                    mui.add_space(SPACE_2);
                }
                show_quiet(&mut mui, check);
            }
        });
}

/// 正常项：标题 + 详情 + 提示，不再重复标注「正常」。
fn show_quiet(ui: &mut MaterialUi<'_>, check: &Check) {
    let variant = design::on_surface_variant(ui);
    ui.horizontal_top(|ui| {
        ui.label(RichText::new("●").size(TEXT_LABEL).color(design::OK));
        ui.add_space(SPACE_2);
        ui.vertical(|ui| {
            ui.label(RichText::new(&check.title).size(TEXT_BODY));
            ui.add_space(SPACE_1);
            ui.label(RichText::new(&check.detail).size(TEXT_LABEL).color(variant));
            if let Some(hint) = &check.hint {
                ui.add_space(SPACE_1);
                ui.label(RichText::new(hint).size(TEXT_LABEL).color(variant));
            }
        });
    });
}
