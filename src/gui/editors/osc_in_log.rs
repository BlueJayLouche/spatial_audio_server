use crate::osc::input::LogEntry;
use egui::Ui;
use std::collections::VecDeque;

pub fn show(ui: &mut Ui, log: &VecDeque<LogEntry>) {
    ui.heading("OSC In Log");
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(200.0)
        .show(ui, |ui| {
            for entry in log.iter() {
                ui.label(format!("[{}] {} {}", entry.from, entry.addr, entry.args));
            }
        });
}
