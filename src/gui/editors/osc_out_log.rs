use crate::osc::output::LogEntry;
use egui::Ui;
use std::collections::VecDeque;

pub fn show(ui: &mut Ui, log: &VecDeque<LogEntry>) {
    ui.heading("OSC Out Log");
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(200.0)
        .show(ui, |ui| {
            for entry in log.iter() {
                let color = if entry.error {
                    crate::gui::theme::LEVEL_RED
                } else {
                    crate::gui::theme::TEXT
                };
                ui.colored_label(color, format!("[{}] {}", entry.target, entry.osc_addr));
            }
        });
}
