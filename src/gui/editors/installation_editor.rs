use crate::installation::{self, Installation};
use crate::project::Installations;
use egui::Ui;

/// Transient UI state for the installation editor.
#[derive(Default)]
pub struct State {
    pub selected: Option<installation::Id>,
    pub new_name: String,
}

/// Returns `true` if any installation was added, removed, or modified.
pub fn show(ui: &mut Ui, state: &mut State, installations: &mut Installations) -> bool {
    let mut changed = false;

    ui.heading("Installations");
    ui.separator();

    let ids: Vec<installation::Id> = installations.keys().copied().collect();
    for id in &ids {
        if let Some(inst) = installations.get(id) {
            let selected = state.selected == Some(*id);
            if ui.selectable_label(selected, &inst.name).clicked() {
                state.selected = Some(*id);
            }
        }
    }

    ui.separator();

    if let Some(sel_id) = state.selected {
        if let Some(inst) = installations.get_mut(&sel_id) {
            ui.horizontal(|ui| {
                ui.label("Name");
                if ui.text_edit_singleline(&mut inst.name).changed() {
                    changed = true;
                }
            });

            ui.label(format!(
                "OSC addr: {}",
                installation::osc_addr_string(&inst.name)
            ));

            let mut min = inst.soundscape.simultaneous_sounds.min;
            let mut max = inst.soundscape.simultaneous_sounds.max;
            ui.horizontal(|ui| {
                ui.label("Simultaneous sounds min");
                if ui.add(egui::DragValue::new(&mut min).range(0..=64)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Simultaneous sounds max");
                if ui.add(egui::DragValue::new(&mut max).range(1..=64)).changed() {
                    changed = true;
                }
            });
            inst.soundscape.simultaneous_sounds.min = min.min(max);
            inst.soundscape.simultaneous_sounds.max = max.max(1);
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("New installation");
        ui.text_edit_singleline(&mut state.new_name);
        if ui.button("Add").clicked() && !state.new_name.is_empty() {
            let next_id = installation::Id(
                installations.keys().map(|k| k.0).max().unwrap_or(0) + 1,
            );
            let name = std::mem::take(&mut state.new_name);
            installations.insert(next_id, Installation { name, ..Default::default() });
            state.selected = Some(next_id);
            changed = true;
        }
    });

    changed
}
