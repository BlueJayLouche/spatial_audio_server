use crate::project::SoundscapeGroups;
use crate::soundscape::group;
use egui::Ui;

/// Transient UI state for the soundscape editor.
#[derive(Default)]
pub struct State {
    pub selected_group: Option<group::Id>,
    pub new_group_name: String,
}

/// Returns `true` if any group was added, removed, or modified.
pub fn show(ui: &mut Ui, state: &mut State, groups: &mut SoundscapeGroups) -> bool {
    let mut changed = false;

    ui.heading("Soundscape Groups");
    ui.separator();

    let ids: Vec<group::Id> = groups.keys().copied().collect();
    for id in &ids {
        if let Some(g) = groups.get(&id) {
            let selected = state.selected_group == Some(*id);
            if ui.selectable_label(selected, &g.name).clicked() {
                state.selected_group = Some(*id);
            }
        }
    }

    ui.separator();

    if let Some(sel_id) = state.selected_group {
        if let Some(g) = groups.get_mut(&sel_id) {
            ui.horizontal(|ui| {
                ui.label("Name");
                if ui.text_edit_singleline(&mut g.name).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Occurrence rate min (ms)");
                let mut v = g.soundscape.occurrence_rate.min.0;
                if ui.add(egui::DragValue::new(&mut v).range(0.0..=f64::MAX).speed(100.0)).changed() {
                    g.soundscape.occurrence_rate.min.0 = v;
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Occurrence rate max (ms)");
                let mut v = g.soundscape.occurrence_rate.max.0;
                if ui.add(egui::DragValue::new(&mut v).range(0.0..=f64::MAX).speed(100.0)).changed() {
                    g.soundscape.occurrence_rate.max.0 = v;
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Simultaneous sounds min");
                if ui.add(egui::DragValue::new(&mut g.soundscape.simultaneous_sounds.min).range(0..=64)).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Simultaneous sounds max");
                if ui.add(egui::DragValue::new(&mut g.soundscape.simultaneous_sounds.max).range(1..=64)).changed() {
                    changed = true;
                }
            });

            if ui.button("Remove group").clicked() {
                groups.remove(&sel_id);
                state.selected_group = None;
                changed = true;
            }
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("New group");
        ui.text_edit_singleline(&mut state.new_group_name);
        if ui.button("Add").clicked() && !state.new_group_name.is_empty() {
            let next_id = group::Id(
                groups.keys().map(|k| k.0).max().unwrap_or(0) + 1,
            );
            let name = std::mem::take(&mut state.new_group_name);
            groups.insert(
                next_id,
                crate::project::SoundscapeGroup {
                    name,
                    soundscape: group::Group::default(),
                },
            );
            state.selected_group = Some(next_id);
            changed = true;
        }
    });

    changed
}
