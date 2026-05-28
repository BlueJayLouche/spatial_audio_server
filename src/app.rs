use crate::audio;
use crate::config::Config;
use crate::gui::{self, SpatialAudioApp};
use crate::project::Project;
use crate::soundscape;
use crate::utils;
use cpal::traits::DeviceTrait;

pub fn run() {
    let assets = utils::assets_dir();

    // Load top-level config (creates default if absent)
    let config: Config = utils::load_from_json_or_default(&assets.join("config.json"));
    let project_slug = config.selected_project_slug.clone();
    let seed = config.project_default.seed;
    let osc_port = config.project_default.osc_input_port;

    // ── Audio setup ───────────────────────────────────────────────────────────

    let host = audio::host();

    let output_device = audio::find_output_device(&host, &config.target_output_device_name)
        .expect("no audio output device found");

    let output_config = best_stream_config(
        &output_device,
        audio::SAMPLE_RATE as u32,
        audio::MAX_CHANNELS as u16,
        cpal::SampleFormat::F32,
        StreamDir::Output,
    );

    let (sound_cmd_tx, sound_cmd_rx) = crossbeam::channel::unbounded::<audio::sound::SoundCommand>();

    // WAV reader thread
    let wav_spawned = audio::source::wav::reader::spawn();

    // Audio output stream (renders silence; mixing wired in a future step)
    let audio_out = audio::output::Model::new(&output_device, &output_config)
        .expect("failed to start audio output stream");

    // Audio input stream (optional — fall back gracefully)
    let audio_in = audio::find_input_device(&host, &config.target_input_device_name)
        .and_then(|dev| {
            let cfg = best_stream_config(
                &dev,
                audio::SAMPLE_RATE as u32,
                2,
                cpal::SampleFormat::F32,
                StreamDir::Input,
            );
            let (tx, _rx) = crossbeam::channel::bounded::<Vec<f32>>(4);
            audio::input::Model::new(&dev, &cfg, tx).ok()
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
            // Create a no-op spawned that immediately exits
            no_op_osc_in()
        });

    let osc_out = crate::osc::output::spawn();

    // ── Audio monitor channel ─────────────────────────────────────────────────

    let (monitor_tx, monitor_rx) = gui::monitor::channel();

    // ── Load project ──────────────────────────────────────────────────────────

    let project = Project::load(&assets, &project_slug);

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
            app._sound_cmd_rx = Some(sound_cmd_rx);
            app._monitor_tx = Some(monitor_tx);
            app.osc_in = Some(osc_in);
            app.osc_out = Some(osc_out);
            app.soundscape = Some(soundscape);
            app.wav_reader = Some(wav_spawned);
            app._audio_in = audio_in;
            app._audio_out = Some(audio_out);

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
                buffer_size: cpal::BufferSize::Default,
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
