use crate::master::Master;
use egui::Ui;

/// Returns `true` if volume or rolloff changed (caller should push to audio thread).
pub fn show(ui: &mut Ui, master: &mut Master, cpu_saving: &mut bool, is_playing: bool) -> bool {
    let mut changed = false;

    ui.heading("Master");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Volume");
        if ui.add(egui::Slider::new(&mut master.volume, 0.0..=1.0).show_value(true)).changed() {
            changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("DBAP rolloff (dB)");
        if ui.add(
            egui::Slider::new(&mut master.dbap_rolloff_db, 0.0..=12.0)
                .step_by(0.1)
                .show_value(true),
        ).changed() {
            changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Realtime latency (ms)");
        let mut ms = master.realtime_source_latency.0;
        if ui.add(egui::DragValue::new(&mut ms).range(0.0..=1000.0).speed(1.0)).changed() {
            master.realtime_source_latency.0 = ms;
        }
    });

    ui.separator();

    changed
}
