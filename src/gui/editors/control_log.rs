use crate::osc::input::ControlMsg;
use egui::Ui;
use std::collections::VecDeque;

pub fn show(ui: &mut Ui, log: &VecDeque<ControlMsg>) {
    ui.heading("Control Log");
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(200.0)
        .show(ui, |ui| {
            for msg in log.iter() {
                let text = match msg {
                    ControlMsg::MasterVolume(v) => format!("MasterVolume {:.3}", v),
                    ControlMsg::SourceVolume { name, volume } => {
                        format!("SourceVolume {} {:.3}", name, volume)
                    }
                    ControlMsg::PlaySoundscape => "PlaySoundscape".into(),
                    ControlMsg::PauseSoundscape => "PauseSoundscape".into(),
                };
                ui.label(text);
            }
        });
}
