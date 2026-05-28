use crate::audio::source;
use crate::project::{Source, SourcesMap};
use egui::Ui;

/// Transient UI state for the source editor.
#[derive(Default)]
pub struct State {
    pub selected: Option<source::Id>,
    pub last_error: Option<String>,
}

pub fn show(ui: &mut Ui, state: &mut State, sources: &mut SourcesMap) {
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
                        }
                        Err(e) => {
                            state.last_error = Some(format!("Could not load \"{stem}\": {e}"));
                        }
                    }
                }
            }
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
            show_source_detail(ui, src);
        }

        ui.add_space(8.0);
        if ui.button("Remove selected").clicked() {
            sources.remove(&sel_id);
            state.selected = None;
        }
    }
}

fn show_source_detail(ui: &mut Ui, src: &mut Source) {
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
            ui.label(format!("Channels: {}..{}", rt.channels.start, rt.channels.end));
        }
    }

    if let Some(role) = &src.audio.role {
        let role_str = match role {
            source::Role::Soundscape(_) => "Soundscape",
            source::Role::Interactive => "Interactive",
            source::Role::Scribbles => "Scribbles",
        };
        ui.label(format!("Role: {role_str}"));
    }
}
