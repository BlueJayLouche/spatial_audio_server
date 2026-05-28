use crate::audio;
use crate::audio::sound;
use crate::config::Config;
use crate::gui::{self, SpatialAudioApp};
use crate::project::Project;
use crate::soundscape;
use crate::utils;
use cpal::traits::{DeviceTrait, HostTrait};

pub fn run() {
    let assets = utils::assets_dir();

    // Load top-level config (creates default if absent)
    let config: Config = utils::load_from_json_or_default(&assets.join("config.json"));
    let project_slug = config.selected_project_slug.clone();
    let seed = config.project_default.seed;
    let osc_port = config.project_default.osc_input_port;

    // ── Audio setup ───────────────────────────────────────────────────────────

    let host = audio::host();

    #[allow(deprecated)]
    let output_devices: Vec<String> = host.output_devices()
        .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    #[allow(deprecated)]
    let input_devices: Vec<String> = host.input_devices()
        .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();

    let output_device = audio::find_output_device(&host, &config.target_output_device_name)
        .expect("no audio output device found");

    let output_config = best_stream_config(
        &output_device,
        audio::SAMPLE_RATE as u32,
        audio::MAX_CHANNELS as u16,
        cpal::SampleFormat::F32,
        StreamDir::Output,
    );

    // ── SoundCommand channel — shared by soundscape (tx) and output (rx) ──────
    //
    // Keep a second sender clone so the GUI can send SetSpeakers and future
    // direct commands without going through the soundscape thread.

    let (sound_cmd_tx, sound_cmd_rx) = crossbeam::channel::unbounded::<audio::sound::SoundCommand>();
    let gui_sound_cmd_tx = sound_cmd_tx.clone();

    // ── WAV reader thread ─────────────────────────────────────────────────────

    let (wav_spawned, wav_rx) = audio::source::wav::reader::spawn();

    // ── Live input channel ────────────────────────────────────────────────────
    //
    // The input callback pushes Vec<f32> chunks here; the output engine drains
    // them each render block.  Bounded at 4 so the callback drops silently when
    // the output thread is slow — no backpressure on the audio thread.

    let (input_tx, input_rx) = crossbeam::channel::bounded::<Vec<f32>>(4);

    // Probe input device config now so we know the channel count before building
    // the output model.  We build the model first so it owns `input_rx`.
    let (input_device_opt, input_cfg_opt) =
        audio::find_input_device(&host, &config.target_input_device_name)
            .map(|dev| {
                let cfg = best_stream_config(
                    &dev,
                    audio::SAMPLE_RATE as u32,
                    2,
                    cpal::SampleFormat::F32,
                    StreamDir::Input,
                );
                (Some(dev), Some(cfg))
            })
            .unwrap_or((None, None));

    let input_channels: usize = input_cfg_opt
        .as_ref()
        .map(|c| c.channels as usize)
        .unwrap_or(2);

    // ── Load project ──────────────────────────────────────────────────────────

    let project = Project::load(&assets, &project_slug);

    // Build initial speaker snapshot for DBAP.
    let initial_speakers: Vec<sound::SpeakerSnapshot> = project.state.speakers.values()
        .map(|s| sound::SpeakerSnapshot {
            point: [s.audio.point.x.0, s.audio.point.y.0],
            channel: s.audio.channel,
        })
        .collect();

    // ── Audio output stream (full mixing engine) ──────────────────────────────

    let audio_out = audio::output::Model::new(
        &output_device,
        &output_config,
        sound_cmd_rx,
        wav_rx,
        input_rx,
        project.state.master.volume,
        project.state.master.dbap_rolloff_db,
        initial_speakers,
        input_channels,
    )
    .expect("failed to start audio output stream");

    // ── Audio input stream (optional) ─────────────────────────────────────────

    let audio_in = input_device_opt.zip(input_cfg_opt).and_then(|(dev, cfg)| {
        audio::input::Model::new(&dev, &cfg, input_tx).ok()
    });

    // ── Soundscape ────────────────────────────────────────────────────────────

    let sound_id_gen = audio::sound::IdGenerator::new();
    let soundscape = soundscape::spawn(
        seed,
        sound_id_gen,
        wav_spawned.handle.clone(),
        sound_cmd_tx,
    );

    // ── OSC ───────────────────────────────────────────────────────────────────

    let osc_in = crate::osc::input::spawn(osc_port)
        .unwrap_or_else(|e| {
            eprintln!("OSC input on port {osc_port} failed: {e} — continuing without OSC input");
            no_op_osc_in()
        });

    let osc_out = crate::osc::output::spawn();

    // ── Audio monitor channel ─────────────────────────────────────────────────

    let (monitor_tx, monitor_rx) = gui::monitor::channel();

    // ── Build and run the GUI ─────────────────────────────────────────────────

    let viewport = project.config.window_width as f32;
    let viewport_h = project.config.window_height as f32;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Spatial Audio Server")
            .with_inner_size([viewport.max(1024.0), viewport_h.max(640.0)]),
        ..Default::default()
    };

    eframe::run_native(
        "Spatial Audio Server",
        options,
        Box::new(move |cc| {
            gui::theme::apply(&cc.egui_ctx);

            let mut app = SpatialAudioApp::default();
            app.assets = assets;
            app.config = config;
            app.project_slug = project_slug;
            app.project = Some((project.state, Default::default()));
            app.audio_monitor_rx = Some(monitor_rx);
            app._monitor_tx = Some(monitor_tx);
            app.audio_cmd_tx = Some(gui_sound_cmd_tx);
            app.osc_in = Some(osc_in);
            app.osc_out = Some(osc_out);
            app.soundscape = Some(soundscape);
            app.wav_reader = Some(wav_spawned);
            app._audio_in = audio_in;
            app._audio_out = Some(audio_out);
            app.output_devices = output_devices;
            app.input_devices = input_devices;

            Ok(Box::new(app))
        }),
    )
    .expect("eframe failed to start");
}

// ── Helpers ───────────────────────────────────────────────────────────────────

enum StreamDir { Input, Output }

/// Select the best `StreamConfig` for a device: prefer our target sample rate
/// and channel count, fall back to the device default.
fn best_stream_config(
    device: &cpal::Device,
    target_hz: u32,
    max_channels: u16,
    sample_format: cpal::SampleFormat,
    dir: StreamDir,
) -> cpal::StreamConfig {
    let supported: Vec<cpal::SupportedStreamConfigRange> = match dir {
        StreamDir::Output => device.supported_output_configs().ok().into_iter().flatten().collect(),
        StreamDir::Input  => device.supported_input_configs().ok().into_iter().flatten().collect(),
    };

    // Try to find a config that supports our target sample rate
    for range in &supported {
        if range.min_sample_rate() <= target_hz
            && range.max_sample_rate() >= target_hz
            && range.sample_format() == sample_format
        {
            let channels = range.channels().min(max_channels);
            return cpal::StreamConfig {
                channels,
                sample_rate: target_hz,
                buffer_size: cpal::BufferSize::Fixed(audio::FRAMES_PER_BUFFER as u32),
            };
        }
    }

    // Fallback: device default config
    let default = match dir {
        StreamDir::Output => device.default_output_config(),
        StreamDir::Input  => device.default_input_config(),
    };
    match default {
        Ok(cfg) => cfg.into(),
        Err(_) => cpal::StreamConfig {
            channels: 2,
            sample_rate: target_hz,
            buffer_size: cpal::BufferSize::Default,
        },
    }
}

fn no_op_osc_in() -> crate::osc::input::Spawned {
    crate::osc::input::Spawned::inert()
}
