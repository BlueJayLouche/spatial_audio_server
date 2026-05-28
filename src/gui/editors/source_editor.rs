use crate::audio::source;
use crate::project::{Source, SourcesMap};
use egui::Ui;

/// Transient UI state for the source editor.
#[derive(Default)]
pub struct State {
    pub selected: Option<source::Id>,
}

pub fn show(ui: &mut Ui, state: &mut State, sources: &mut SourcesMap) {
    ui.heading("Sources");
    ui.separator();

    let ids: Vec<source::Id> = sources.keys().copied().collect();
    for id in &ids {
        if let Some(src) = sources.get(&id) {
            let selected = state.selected == Some(*id);
            let kind_str = match &src.audio.kind {
                source::Kind::Wav(w) => format!("WAV {}", w.path.file_name()
                    .and_then(|n| n.to_str()).unwrap_or("?")),
                source::Kind::Realtime(_) => "Realtime".to_string(),
            };
            let label = format!("{} — {}", src.name, kind_str);
            if ui.selectable_label(selected, &label).clicked() {
                state.selected = Some(*id);
            }
        }
    }

    ui.separator();

    if let Some(sel_id) = state.selected {
        if let Some(src) = sources.get_mut(&sel_id) {
            show_source_detail(ui, src);
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
            ui.label(format!("Channels: {}", w.channels));
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
        ui.label(format!("Role: {}", role_str));
    }
}
