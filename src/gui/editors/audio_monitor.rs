use crate::audio::detection::AudioFrameData;
use crate::audio::speaker;
use crate::gui::monitor::ActiveSoundMonitor;
use crate::audio::sound;
use egui::Ui;
use fxhash::FxHashMap;

pub fn show(
    ui: &mut Ui,
    frame: &AudioFrameData,
    active_sounds: &FxHashMap<sound::Id, ActiveSoundMonitor>,
    speakers: &crate::project::Speakers,
) {
    ui.heading("Audio Monitor");
    ui.separator();

    // Master peak bar
    ui.horizontal(|ui| {
        ui.label("Master peak");
        let level_color = crate::gui::theme::level_color(frame.avg_peak);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(200.0, 14.0),
            egui::Sense::hover(),
        );
        if ui.is_rect_visible(rect) {
            let filled = egui::Rect::from_min_max(
                rect.min,
                egui::pos2(rect.min.x + rect.width() * frame.avg_peak, rect.max.y),
            );
            ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(40, 40, 50));
            ui.painter().rect_filled(filled, 2.0, level_color);
        }
    });

    ui.label(format!("avg RMS: {:.3}", frame.avg_rms));

    ui.separator();
    ui.label(format!("Active sounds: {}", active_sounds.len()));

    if !speakers.is_empty() {
        ui.separator();
        ui.label("Speakers");
        for (i, data) in frame.speakers.iter().enumerate() {
            let color = crate::gui::theme::level_color(data.peak);
            ui.colored_label(color, format!("Ch{}: peak={:.3} rms={:.3}", i + 1, data.peak, data.rms));
        }
    }
}
