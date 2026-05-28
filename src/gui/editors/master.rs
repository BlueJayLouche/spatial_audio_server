use crate::master::Master;
use egui::Ui;

pub fn show(ui: &mut Ui, master: &mut Master, cpu_saving: &mut bool, is_playing: bool) {
    ui.heading("Master");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Volume");
        ui.add(egui::Slider::new(&mut master.volume, 0.0..=1.0).show_value(true));
    });

    ui.horizontal(|ui| {
        ui.label("DBAP rolloff (dB)");
        ui.add(
            egui::Slider::new(&mut master.dbap_rolloff_db, 0.0..=12.0)
                .step_by(0.1)
                .show_value(true),
        );
    });

    ui.horizontal(|ui| {
        ui.label("Realtime latency (ms)");
        let mut ms = master.realtime_source_latency.0;
        if ui.add(egui::DragValue::new(&mut ms).range(0.0..=1000.0).speed(1.0)).changed() {
            master.realtime_source_latency.0 = ms;
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        let label = if is_playing { "Pause Soundscape" } else { "Play Soundscape" };
        let _ = ui.button(label); // Phase 6 will wire this to soundscape thread
    });

    ui.checkbox(cpu_saving, "CPU saving mode");
}
