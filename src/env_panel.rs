use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, RichText};

use crate::steam::env::{self, Check, Level, Report};

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

    pub fn show_checks(&mut self, ui: &mut egui::Ui) {
        let State::Ready(report) = &self.state else {
            return;
        };
        for check in &report.checks {
            show_check(ui, check);
        }
    }
}

fn level_color(level: Level, ui: &egui::Ui) -> egui::Color32 {
    match level {
        Level::Ok => crate::ui::OK,
        Level::Warn => crate::ui::WARN,
        Level::Error => ui.visuals().error_fg_color,
    }
}

fn show_check(ui: &mut egui::Ui, check: &Check) {
    ui.horizontal(|ui| {
        ui.colored_label(level_color(check.level, ui), "●");
        ui.label(RichText::new(&check.title).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(check.level.label())
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(&check.detail).small());
            if let Some(hint) = &check.hint {
                ui.label(
                    RichText::new(hint)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
    });
    ui.add_space(4.0);
}
