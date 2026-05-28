pub mod editors;
pub mod monitor;
pub mod theme;

use crate::audio::detection::AudioFrameData;
use crate::audio::sound;
use crate::osc::input::ControlMsg;
use crate::project;
use crossbeam::channel::Sender;
use fxhash::FxHashMap;
use monitor::ActiveSoundMonitor;
use std::collections::VecDeque;
use std::path::PathBuf;

// ── Per-project editor state ──────────────────────────────────────────────────

#[derive(Default)]
pub struct ProjectEditorState {
    pub installation: editors::installation_editor::State,
    pub speaker: editors::speaker_editor::State,
    pub source: editors::source_editor::State,
    pub soundscape: editors::soundscape_editor::State,
    pub project_editor: editors::project_editor::State,
}

// ── Panel visibility ──────────────────────────────────────────────────────────

pub struct PanelVisibility {
    pub project: bool,
    pub master: bool,
    pub installation: bool,
    pub speaker: bool,
    pub source: bool,
    pub soundscape: bool,
    pub osc_in_log: bool,
    pub osc_out_log: bool,
    pub control_log: bool,
    pub audio_monitor: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        PanelVisibility {
            project: false,
            master: true,
            installation: false,
            speaker: false,
            source: false,
            soundscape: false,
            osc_in_log: false,
            osc_out_log: false,
            control_log: false,
            audio_monitor: true,
        }
    }
}

// ── Top-level eframe app ──────────────────────────────────────────────────────

pub struct SpatialAudioApp {
    // GUI / project state
    pub project: Option<(project::State, ProjectEditorState)>,
    pub cpu_saving_mode: bool,
    pub soundscape_playing: bool,
    pub osc_in_log: VecDeque<crate::osc::input::LogEntry>,
    pub osc_out_log: VecDeque<crate::osc::output::LogEntry>,
    pub control_log: VecDeque<ControlMsg>,
    pub audio_frame: AudioFrameData,
    pub active_sounds: FxHashMap<sound::Id, ActiveSoundMonitor>,
    pub panel_visibility: PanelVisibility,

    // Thread handles (populated during initialisation, taken on exit)
    pub osc_in: Option<crate::osc::input::Spawned>,
    pub osc_out: Option<crate::osc::output::Spawned>,
    pub soundscape: Option<crate::soundscape::Soundscape>,
    pub wav_reader: Option<crate::audio::source::wav::reader::Spawned>,
    /// Audio input stream — kept alive while held, dropped on exit.
    pub _audio_in: Option<crate::audio::input::Model>,
    /// Audio output stream — kept alive while held, dropped on exit.
    pub _audio_out: Option<crate::audio::output::Model>,

    // Incoming data channels
    pub audio_monitor_rx: Option<monitor::MsgReceiver>,

    // Sender to the audio output thread for speaker updates and future direct commands
    pub audio_cmd_tx: Option<Sender<sound::SoundCommand>>,
    pub _monitor_tx: Option<monitor::MsgSender>,

    // Persistence
    pub assets: PathBuf,
    pub config: crate::config::Config,
    pub project_slug: String,

    // Available audio devices (enumerated at startup, used by the device selector)
    pub output_devices: Vec<String>,
    pub input_devices: Vec<String>,
}

impl Default for SpatialAudioApp {
    fn default() -> Self {
        SpatialAudioApp {
            project: None,
            cpu_saving_mode: false,
            soundscape_playing: false,
            osc_in_log: VecDeque::new(),
            osc_out_log: VecDeque::new(),
            control_log: VecDeque::new(),
            audio_frame: Default::default(),
            active_sounds: Default::default(),
            panel_visibility: Default::default(),
            osc_in: None,
            osc_out: None,
            soundscape: None,
            wav_reader: None,
            _audio_in: None,
            _audio_out: None,
            audio_monitor_rx: None,
            audio_cmd_tx: None,
            _monitor_tx: None,
            assets: PathBuf::from("assets"),
            config: Default::default(),
            project_slug: String::new(),
            output_devices: Vec::new(),
            input_devices: Vec::new(),
        }
    }
}

impl SpatialAudioApp {
    fn poll_channels(&mut self) {
        // Audio monitor
        if let Some(rx) = self.audio_monitor_rx.as_ref() {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    monitor::AudioMonitorMsg::Frame(frame) => {
                        self.audio_frame = frame;
                    }
                    monitor::AudioMonitorMsg::SoundUpdate { id, monitor } => {
                        self.active_sounds.insert(id, monitor);
                    }
                    monitor::AudioMonitorMsg::SoundEnded(id) => {
                        self.active_sounds.remove(&id);
                    }
                }
            }
        }

        // Collect OSC messages into local vecs first to avoid borrow conflicts.
        let osc_in_entries: Vec<_> = self.osc_in.as_ref()
            .map(|o| std::iter::from_fn(|| o.log_rx.try_recv().ok()).collect())
            .unwrap_or_default();
        let controls: Vec<_> = self.osc_in.as_ref()
            .map(|o| std::iter::from_fn(|| o.control_rx.try_recv().ok()).collect())
            .unwrap_or_default();
        let osc_out_entries: Vec<_> = self.osc_out.as_ref()
            .map(|o| std::iter::from_fn(|| o.log_rx.try_recv().ok()).collect())
            .unwrap_or_default();

        let osc_in_log_limit = self.config.project_default.osc_input_log_limit;
        let control_log_limit = self.config.project_default.control_log_limit;
        let osc_out_log_limit = self.config.project_default.osc_output_log_limit;

        for entry in osc_in_entries {
            self.osc_in_log.push_front(entry);
            while self.osc_in_log.len() > osc_in_log_limit { self.osc_in_log.pop_back(); }
        }
        for ctrl in controls {
            self.apply_control(&ctrl);
            self.control_log.push_front(ctrl);
            while self.control_log.len() > control_log_limit { self.control_log.pop_back(); }
        }
        for entry in osc_out_entries {
            self.osc_out_log.push_front(entry);
            while self.osc_out_log.len() > osc_out_log_limit { self.osc_out_log.pop_back(); }
        }
    }

    fn apply_control(&mut self, ctrl: &crate::osc::input::ControlMsg) {
        use crate::osc::input::ControlMsg::*;
        match ctrl {
            MasterVolume(v) => {
                if let Some((state, _)) = self.project.as_mut() {
                    state.master.volume = *v;
                }
            }
            SourceVolume { name, volume } => {
                if let Some((state, _)) = self.project.as_mut() {
                    for src in state.sources.map.values_mut() {
                        if src.name == *name {
                            src.audio.volume = *volume;
                        }
                    }
                }
            }
            PlaySoundscape => {
                self.soundscape_playing = true;
                if let Some(ss) = self.soundscape.as_ref() { ss.play(); }
            }
            PauseSoundscape => {
                self.soundscape_playing = false;
                if let Some(ss) = self.soundscape.as_ref() { ss.pause(); }
            }
        }
    }

    fn save_project(&self) {
        if let Some((state, _)) = self.project.as_ref() {
            if self.project_slug.is_empty() { return; }
            if let Err(e) = crate::project::Project::save_parts(
                &self.assets,
                &self.project_slug,
                state,
                &self.config.project_default,
            ) {
                eprintln!("failed to save project: {e}");
            }
        }
    }

    fn save_config(&self) {
        let path = self.assets.join("config.json");
        if let Err(e) = crate::utils::save_to_json(&path, &self.config) {
            eprintln!("failed to save config: {e}");
        }
    }

    /// Send the current speaker list to the audio output thread for DBAP panning.
    pub fn send_speakers(&self) {
        let Some(tx) = self.audio_cmd_tx.as_ref() else { return };
        let Some((state, _)) = self.project.as_ref() else { return };
        let snapshots: Vec<sound::SpeakerSnapshot> = state.speakers.values()
            .map(|s| sound::SpeakerSnapshot {
                point: [s.audio.point.x.0, s.audio.point.y.0],
                channel: s.audio.channel,
            })
            .collect();
        let _ = tx.send(sound::SoundCommand::SetSpeakers(snapshots));
    }

    fn shutdown_threads(&mut self) {
        // Soundscape first so it stops sending commands before audio output goes down
        if let Some(ss) = self.soundscape.take() {
            ss.exit();
        }
        // WAV reader
        if let Some(wav) = self.wav_reader.take() {
            let _ = wav.exit();
        }
        // OSC threads
        if let Some(osc_in) = self.osc_in.take() {
            let _ = osc_in.exit().join();
        }
        if let Some(osc_out) = self.osc_out.take() {
            let _ = osc_out.join();
        }
        // Audio streams drop here (cpal stops on Drop)
        self._audio_in = None;
        self._audio_out = None;
    }
}

impl eframe::App for SpatialAudioApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_channels();
        if self.cpu_saving_mode {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn on_exit(&mut self) {
        // Save before shutdown: the soundscape thread mutates only its own Model,
        // not SpatialAudioApp::project, so project state is stable here.
        // If future phases push state back to the GUI, reverse this order.
        self.save_project();
        self.save_config();
        self.shutdown_threads();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("side_panel")
            .default_size(theme::SIDE_PANEL_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    side_panel(ui, self);
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            floorplan_canvas(ui, self);
        });
    }
}

// ── Side panel ────────────────────────────────────────────────────────────────

fn side_panel(ui: &mut egui::Ui, app: &mut SpatialAudioApp) {
    // Project
    egui::CollapsingHeader::new("Project")
        .default_open(app.panel_visibility.project)
        .show(ui, |ui| {
            let name = app.project.as_ref().map(|(s, _)| s.name.clone());
            ui.label(format!("Project: {}", name.as_deref().unwrap_or("(none)")));
            if name.is_some() && ui.button("Save project").clicked() {
                // Phase 6: wire to disk
            }
        });

    // Master
    egui::CollapsingHeader::new("Master")
        .default_open(app.panel_visibility.master)
        .show(ui, |ui| {
            if let Some((state, _)) = app.project.as_mut() {
                editors::master::show(
                    ui,
                    &mut state.master,
                    &mut app.cpu_saving_mode,
                    app.soundscape_playing,
                );
            } else {
                ui.label("No project loaded.");
            }
        });

    // Devices
    egui::CollapsingHeader::new("Devices")
        .default_open(false)
        .show(ui, |ui| {
            let changed = editors::device_editor::show(
                ui,
                &mut app.config,
                &app.output_devices,
                &app.input_devices,
            );
            if changed {
                app.save_config();
            }
        });

    // Installations
    egui::CollapsingHeader::new("Installations")
        .default_open(app.panel_visibility.installation)
        .show(ui, |ui| {
            if let Some((state, editor)) = app.project.as_mut() {
                editors::installation_editor::show(
                    ui,
                    &mut editor.installation,
                    &mut state.installations,
                );
            } else {
                ui.label("No project loaded.");
            }
        });

    // Speakers
    egui::CollapsingHeader::new("Speakers")
        .default_open(app.panel_visibility.speaker)
        .show(ui, |ui| {
            if let Some((state, editor)) = app.project.as_mut() {
                let changed = editors::speaker_editor::show(
                    ui, &mut editor.speaker, &mut state.speakers, 128,
                );
                if changed {
                    app.send_speakers();
                    app.save_project();
                }
            } else {
                ui.label("No project loaded.");
            }
        });

    // Sources
    egui::CollapsingHeader::new("Sources")
        .default_open(app.panel_visibility.source)
        .show(ui, |ui| {
            if let Some((state, editor)) = app.project.as_mut() {
                editors::source_editor::show(
                    ui,
                    &mut editor.source,
                    &mut state.sources.map,
                    &state.installations,
                    &state.soundscape_groups,
                );
            } else {
                ui.label("No project loaded.");
            }
        });

    // Soundscape
    egui::CollapsingHeader::new("Soundscape")
        .default_open(app.panel_visibility.soundscape)
        .show(ui, |ui| {
            if let Some((state, editor)) = app.project.as_mut() {
                editors::soundscape_editor::show(
                    ui,
                    &mut editor.soundscape,
                    &mut state.soundscape_groups,
                );
            } else {
                ui.label("No project loaded.");
            }
        });

    // Logs
    egui::CollapsingHeader::new("OSC In Log")
        .default_open(app.panel_visibility.osc_in_log)
        .show(ui, |ui| {
            editors::osc_in_log::show(ui, &app.osc_in_log);
        });

    egui::CollapsingHeader::new("OSC Out Log")
        .default_open(app.panel_visibility.osc_out_log)
        .show(ui, |ui| {
            editors::osc_out_log::show(ui, &app.osc_out_log);
        });

    egui::CollapsingHeader::new("Control Log")
        .default_open(app.panel_visibility.control_log)
        .show(ui, |ui| {
            editors::control_log::show(ui, &app.control_log);
        });

    // Audio monitor
    egui::CollapsingHeader::new("Audio Monitor")
        .default_open(app.panel_visibility.audio_monitor)
        .show(ui, |ui| {
            let speakers = app.project.as_ref().map(|(s, _)| &s.speakers);
            editors::audio_monitor::show(
                ui,
                &app.audio_frame,
                &app.active_sounds,
                speakers.unwrap_or(&Default::default()),
            );
        });
}

// ── Floorplan canvas ──────────────────────────────────────────────────────────

fn floorplan_canvas(ui: &mut egui::Ui, app: &SpatialAudioApp) {
    let available = ui.available_rect_before_wrap();
    let (rect, _) = ui.allocate_exact_size(available.size(), egui::Sense::drag());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::DARK_BG);

    let Some((state, _)) = app.project.as_ref() else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No project — use Project panel to load or create one",
            egui::FontId::proportional(16.0),
            theme::DIM_TEXT,
        );
        return;
    };

    let (world_min, world_max) = speaker_bounds(&state.speakers);
    let canvas_w = rect.width();
    let canvas_h = rect.height();
    let world_w = (world_max[0] - world_min[0]).max(1.0) as f32;
    let world_h = (world_max[1] - world_min[1]).max(1.0) as f32;
    let scale = (canvas_w / world_w * 0.85).min(canvas_h / world_h * 0.85);
    let cx = rect.center().x - (world_min[0] + world_max[0]) as f32 * 0.5 * scale;
    let cy = rect.center().y - (world_min[1] + world_max[1]) as f32 * 0.5 * scale;

    let to_canvas = |x: f64, y: f64| -> egui::Pos2 {
        egui::pos2(x as f32 * scale + cx, y as f32 * scale + cy)
    };

    // Speakers
    for spk in state.speakers.values() {
        let pos = to_canvas(spk.audio.point.x.0, spk.audio.point.y.0);
        painter.circle_filled(pos, theme::FLOORPLAN_SPEAKER_RADIUS, theme::ACCENT);
        painter.text(
            egui::pos2(pos.x, pos.y + theme::FLOORPLAN_SPEAKER_RADIUS + 4.0),
            egui::Align2::CENTER_TOP,
            format!("Ch{}", spk.audio.channel + 1),
            egui::FontId::monospace(10.0),
            theme::DIM_TEXT,
        );
    }

    // Active sounds
    for mon in app.active_sounds.values() {
        let pos = to_canvas(mon.position.point.x.0, mon.position.point.y.0);
        let col = theme::level_color(mon.peak);
        painter.circle_filled(pos, theme::FLOORPLAN_SOUND_RADIUS, col);
        painter.circle_stroke(
            pos,
            theme::FLOORPLAN_SOUND_RADIUS,
            egui::Stroke::new(1.0, theme::TEXT),
        );
    }
}

fn speaker_bounds(speakers: &crate::project::Speakers) -> ([f64; 2], [f64; 2]) {
    let mut min = [f64::MAX, f64::MAX];
    let mut max = [f64::MIN, f64::MIN];
    for spk in speakers.values() {
        let x = spk.audio.point.x.0;
        let y = spk.audio.point.y.0;
        if x < min[0] { min[0] = x; }
        if y < min[1] { min[1] = y; }
        if x > max[0] { max[0] = x; }
        if y > max[1] { max[1] = y; }
    }
    if min[0] == f64::MAX {
        return ([-5.0, -5.0], [5.0, 5.0]);
    }
    (min, max)
}
