use crate::audio::speaker;
use crate::geom::Point2;
use crate::metres::Metres;
use crate::project::{Installations, Speakers};
use egui::Ui;

/// Transient UI state for the speaker editor.
#[derive(Default)]
pub struct State {
    pub selected: Option<speaker::Id>,
    pub new_name: String,
}

/// Returns `true` if any speaker was added, removed, or had a field changed.
pub fn show(
    ui: &mut Ui,
    state: &mut State,
    speakers: &mut Speakers,
    installations: &Installations,
    num_channels: usize,
) -> bool {
    let mut changed = false;

    ui.heading("Speakers");
    ui.separator();

    let ids: Vec<speaker::Id> = speakers.keys().copied().collect();
    for id in &ids {
        if let Some(spk) = speakers.get(&id) {
            let selected = state.selected == Some(*id);
            let label = format!("Ch{} — {}", spk.audio.channel + 1, spk.name);
            if ui.selectable_label(selected, &label).clicked() {
                state.selected = Some(*id);
            }
        }
    }

    ui.separator();

    if let Some(sel_id) = state.selected {
        if let Some(spk) = speakers.get_mut(&sel_id) {
            ui.horizontal(|ui| {
                ui.label("Name");
                if ui.text_edit_singleline(&mut spk.name).changed() {
                    changed = true;
                }
            });

            let ch_max = num_channels.max(1);
            ui.horizontal(|ui| {
                ui.label("Channel (1-based)");
                let mut ch = spk.audio.channel + 1;
                if ui.add(egui::DragValue::new(&mut ch).range(1..=ch_max)).changed() {
                    spk.audio.channel = ch.saturating_sub(1);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("X (m)");
                let mut x = spk.audio.point.x.0;
                if ui.add(egui::DragValue::new(&mut x).speed(0.01)).changed() {
                    spk.audio.point.x = Metres(x);
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Y (m)");
                let mut y = spk.audio.point.y.0;
                if ui.add(egui::DragValue::new(&mut y).speed(0.01)).changed() {
                    spk.audio.point.y = Metres(y);
                    changed = true;
                }
            });

            // Installation assignment
            ui.add_space(4.0);
            ui.label("Installations (check to assign):");
            let mut inst_ids: Vec<_> = installations.keys().copied().collect();
            inst_ids.sort_by_key(|i| i.0);
            for inst_id in inst_ids {
                if let Some(inst) = installations.get(&inst_id) {
                    let mut enabled = spk.audio.installations.contains(&inst_id);
                    if ui.checkbox(&mut enabled, &inst.name).changed() {
                        if enabled {
                            spk.audio.installations.insert(inst_id);
                        } else {
                            spk.audio.installations.remove(&inst_id);
                        }
                        changed = true;
                    }
                }
            }

            ui.add_space(4.0);
            if ui.button("Remove speaker").clicked() {
                speakers.remove(&sel_id);
                state.selected = None;
                changed = true;
            }
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("New speaker");
        ui.text_edit_singleline(&mut state.new_name);
        if ui.button("Add").clicked() && !state.new_name.is_empty() {
            let next_id = speaker::Id(
                speakers.keys().map(|k| k.0).max().unwrap_or(0) + 1,
            );
            let name = std::mem::take(&mut state.new_name);
            speakers.insert(
                next_id,
                crate::project::Speaker {
                    name,
                    audio: speaker::Speaker {
                        point: Point2::new(Metres(0.0), Metres(0.0)),
                        channel: 0,
                        installations: Default::default(),
                    },
                },
            );
            state.selected = Some(next_id);
            changed = true;
        }
    });

    changed
}
