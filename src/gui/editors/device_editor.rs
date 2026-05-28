use crate::config::Config;
use egui::Ui;

pub fn show(
    ui: &mut Ui,
    config: &mut Config,
    output_devices: &[String],
    input_devices: &[String],
) -> bool {
    let mut changed = false;

    ui.label("Changes take effect after restart.");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Output:");
        let out_label = if config.target_output_device_name.is_empty() {
            "(System Default)".to_string()
        } else {
            config.target_output_device_name.clone()
        };
        egui::ComboBox::from_id_salt("output_device_combo")
            .selected_text(out_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(
                    config.target_output_device_name.is_empty(),
                    "(System Default)",
                ).clicked() {
                    config.target_output_device_name.clear();
                    changed = true;
                }
                for name in output_devices {
                    let selected = &config.target_output_device_name == name;
                    if ui.selectable_label(selected, name).clicked() {
                        config.target_output_device_name = name.clone();
                        changed = true;
                    }
                }
            });
    });

    ui.horizontal(|ui| {
        ui.label("Input: ");
        let in_label = if config.target_input_device_name.is_empty() {
            "(System Default)".to_string()
        } else {
            config.target_input_device_name.clone()
        };
        egui::ComboBox::from_id_salt("input_device_combo")
            .selected_text(in_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(
                    config.target_input_device_name.is_empty(),
                    "(System Default)",
                ).clicked() {
                    config.target_input_device_name.clear();
                    changed = true;
                }
                for name in input_devices {
                    let selected = &config.target_input_device_name == name;
                    if ui.selectable_label(selected, name).clicked() {
                        config.target_input_device_name = name.clone();
                        changed = true;
                    }
                }
            });
    });

    changed
}
