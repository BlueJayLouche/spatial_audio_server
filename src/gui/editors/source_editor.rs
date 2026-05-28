use crate::audio::source;
use crate::project::{Installations, Source, SoundscapeGroups, SourcesMap};
use egui::Ui;

/// Transient UI state for the source editor.
#[derive(Default)]
pub struct State {
    pub selected: Option<source::Id>,
    pub last_error: Option<String>,
}

/// Returns `true` if any source was added, removed, or had its soundscape constraints changed.
pub fn show(
    ui: &mut Ui,
    state: &mut State,
    sources: &mut SourcesMap,
    installations: &Installations,
    groups: &SoundscapeGroups,
) -> bool {
    let mut changed = false;
    ui.heading("Sources");
    ui.separator();

    // ── Add buttons ───────────────────────────────────────────────────────────

    ui.horizontal(|ui| {
        if ui.button("Add WAV…").clicked() {
            let picked = rfd::FileDialog::new()
                .add_filter("WAV audio", &["wav"])
                .pick_files();

            if let Some(paths) = picked {
                for path in paths {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("source")
                        .to_string();

                    match source::wav::Wav::from_path(path) {
                        Ok(wav) => {
                            let next_id = source::Id(
                                sources.keys().map(|k| k.0).max().map(|m| m + 1).unwrap_or(0),
                            );
                            sources.insert(next_id, Source {
                                name: stem,
                                audio: source::Source {
                                    kind: source::Kind::Wav(wav),
                                    role: Some(source::Role::Soundscape(
                                        source::Soundscape::default(),
                                    )),
                                    spread: source::default::SPREAD,
                                    channel_radians: source::default::CHANNEL_RADIANS,
                                    volume: source::default::VOLUME,
                                    muted: false,
                                },
                            });
                            state.last_error = None;
                            changed = true;
                        }
                        Err(e) => {
                            state.last_error = Some(format!("Could not load \"{stem}\": {e}"));
                        }
                    }
                }
            }
        }

        if ui.button("Add Realtime Input").clicked() {
            let next_id = source::Id(
                sources.keys().map(|k| k.0).max().map(|m| m + 1).unwrap_or(0),
            );
            sources.insert(next_id, Source {
                name: format!("Input {}", next_id.0),
                audio: source::Source {
                    kind: source::Kind::Realtime(source::realtime::Realtime::default()),
                    role: Some(source::Role::Soundscape(source::Soundscape::default())),
                    spread: source::default::SPREAD,
                    channel_radians: source::default::CHANNEL_RADIANS,
                    volume: source::default::VOLUME,
                    muted: false,
                },
            });
            state.last_error = None;
            changed = true;
        }
    });

    if let Some(err) = &state.last_error {
        ui.colored_label(egui::Color32::RED, err);
    }

    ui.separator();

    // ── Source list ───────────────────────────────────────────────────────────

    let ids: Vec<source::Id> = sources.keys().copied().collect();

    if ids.is_empty() {
        ui.label("No sources — click \"Add WAV\u{2026}\" to load files.");
    }

    for id in &ids {
        if let Some(src) = sources.get(id) {
            let selected = state.selected == Some(*id);
            let kind_str = match &src.audio.kind {
                source::Kind::Wav(w) => format!(
                    "WAV {}",
                    w.path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                ),
                source::Kind::Realtime(_) => "Realtime".to_string(),
            };
            let label = format!("{} — {}", src.name, kind_str);
            if ui.selectable_label(selected, &label).clicked() {
                state.selected = Some(*id);
            }
        }
    }

    ui.separator();

    // ── Detail view for selected source ───────────────────────────────────────

    if let Some(sel_id) = state.selected {
        if let Some(src) = sources.get_mut(&sel_id) {
            if show_source_detail(ui, src, installations, groups) {
                changed = true;
            }
        }

        ui.add_space(8.0);
        if ui.button("Remove selected").clicked() {
            sources.remove(&sel_id);
            state.selected = None;
            changed = true;
        }
    }

    changed
}

fn show_source_detail(
    ui: &mut Ui,
    src: &mut Source,
    installations: &Installations,
    groups: &SoundscapeGroups,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut src.name);
    });

    ui.horizontal(|ui| {
        ui.label("Volume");
        ui.add(egui::Slider::new(&mut src.audio.volume, 0.0..=1.0));
    });

    ui.checkbox(&mut src.audio.muted, "Muted");

    match &src.audio.kind {
        source::Kind::Wav(w) => {
            ui.label(format!("Path: {}", w.path.display()));
            ui.label(format!(
                "Duration: {:.1}s  ·  {} ch  ·  {} Hz",
                w.duration as f64 / w.sample_hz,
                w.channels,
                w.sample_hz as u32,
            ));
        }
        source::Kind::Realtime(rt) => {
            ui.label(format!("Input channels: {}..{}", rt.channels.start, rt.channels.end));
        }
    }

    // ── Soundscape role constraints ───────────────────────────────────────────

    if let Some(role) = src.audio.role.as_mut() {
        let role_str = match role {
            source::Role::Soundscape(_) => "Soundscape",
            source::Role::Interactive => "Interactive",
            source::Role::Scribbles => "Scribbles",
        };
        ui.label(format!("Role: {role_str}"));

        if let source::Role::Soundscape(sc) = role {
            ui.add_space(6.0);

            // Installations
            ui.label("Installations (check to enable):");
            let mut inst_ids: Vec<_> = installations.keys().copied().collect();
            inst_ids.sort_by_key(|i| i.0);
            for inst_id in inst_ids {
                if let Some(inst) = installations.get(&inst_id) {
                    let mut enabled = sc.installations.contains(&inst_id);
                    if ui.checkbox(&mut enabled, &inst.name).changed() {
                        if enabled { sc.installations.insert(inst_id); } else { sc.installations.remove(&inst_id); }
                        changed = true;
                    }
                }
            }

            ui.add_space(4.0);

            // Groups
            ui.label("Soundscape groups (check to enable):");
            if groups.is_empty() {
                ui.weak("No groups — add one in the Soundscape panel.");
            } else {
                let mut group_ids: Vec<_> = groups.keys().copied().collect();
                group_ids.sort_by_key(|g| g.0);
                for gid in group_ids {
                    if let Some(g) = groups.get(&gid) {
                        let mut enabled = sc.groups.contains(&gid);
                        if ui.checkbox(&mut enabled, &g.name).changed() {
                            if enabled { sc.groups.insert(gid); } else { sc.groups.remove(&gid); }
                            changed = true;
                        }
                    }
                }
            }

            ui.add_space(4.0);

            // Simultaneous sounds
            ui.horizontal(|ui| {
                ui.label("Simultaneous min");
                if ui.add(egui::DragValue::new(&mut sc.simultaneous_sounds.min).range(0..=32)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Simultaneous max");
                if ui.add(egui::DragValue::new(&mut sc.simultaneous_sounds.max).range(1..=32)).changed() {
                    changed = true;
                }
            });
            sc.simultaneous_sounds.min = sc.simultaneous_sounds.min.min(sc.simultaneous_sounds.max);
        }
    }

    changed
}
