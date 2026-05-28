use crate::project::SoundscapeGroups;
use crate::soundscape::group;
use egui::Ui;

/// Transient UI state for the soundscape editor.
#[derive(Default)]
pub struct State {
    pub selected_group: Option<group::Id>,
    pub new_group_name: String,
}

pub fn show(ui: &mut Ui, state: &mut State, groups: &mut SoundscapeGroups) {
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
                ui.text_edit_singleline(&mut g.name);
            });

            ui.horizontal(|ui| {
                ui.label("Occurrence rate min (ms)");
                let mut v = g.soundscape.occurrence_rate.min.0;
                if ui.add(egui::DragValue::new(&mut v).range(0.0..=f64::MAX).speed(100.0)).changed() {
                    g.soundscape.occurrence_rate.min.0 = v;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Occurrence rate max (ms)");
                let mut v = g.soundscape.occurrence_rate.max.0;
                if ui.add(egui::DragValue::new(&mut v).range(0.0..=f64::MAX).speed(100.0)).changed() {
                    g.soundscape.occurrence_rate.max.0 = v;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Simultaneous sounds min");
                ui.add(egui::DragValue::new(&mut g.soundscape.simultaneous_sounds.min).range(0..=64));
            });

            ui.horizontal(|ui| {
                ui.label("Simultaneous sounds max");
                ui.add(egui::DragValue::new(&mut g.soundscape.simultaneous_sounds.max).range(1..=64));
            });

            if ui.button("Remove group").clicked() {
                groups.remove(&sel_id);
                state.selected_group = None;
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
        }
    });
}
