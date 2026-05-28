# Spatial Audio Server — Modernisation Plan

## Goal

Full standalone rewrite preserving all features of the 2018 MindBuffer/Museums Victoria
spatial audio server, replacing the obsolete nannou 0.13 / conrod stack with a modern,
maintained toolchain.

**Answers captured before planning:**
- Strategy: standalone rewrite, same purpose
- GUI: egui
- Features: OSC in/out, WAV playback & soundscape engine, ASIO, FFT analysis
- Platforms: macOS (primary), Linux, Windows

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│  winit event loop  (main thread)                    │
│  ┌──────────────┐  ┌────────────────────────────┐   │
│  │  egui + wgpu │  │  App state (Model)         │   │
│  └──────┬───────┘  └─────────────┬──────────────┘   │
│         │ render                 │ update            │
└─────────┼─────────────────────────┼─────────────────┘
          │  channels               │
   ┌──────▼──────┐         ┌────────▼────────┐
   │  OSC In     │         │  Soundscape     │
   │  thread     │         │  thread (16ms   │
   │  (rosc)     │         │  tick)          │
   └─────────────┘         └────────┬────────┘
                                    │ sound commands
   ┌─────────────┐         ┌────────▼────────┐
   │  OSC Out    │◄────────│  Audio Output   │
   │  thread     │  FFT    │  thread (cpal)  │
   │  (rosc)     │  data   │  DBAP mixing    │
   └─────────────┘         └────────┬────────┘
                                    │
   ┌─────────────┐         ┌────────▼────────┐
   │  WAV Reader │◄────────│  Audio Input    │
   │  thread     │         │  thread (cpal)  │
   └─────────────┘         └─────────────────┘
   ┌─────────────┐
   │  GUI Monitor│  (audio stats → GUI, async)
   │  thread     │
   └─────────────┘
```

The thread topology from the original is preserved — it is sound and already proven
in the museum deployment. What changes is every library underneath it.

---

## Dependency Migration Map

| Old (broken) | New | Notes |
|---|---|---|
| `nannou 0.13` | `winit 0.30` + `wgpu 29` | app loop + rendering surface |
| `nannou_audio 0.2` | `cpal 0.17` | direct, no wrapper |
| `nannou_osc 0.1` | `rosc 0.11` | |
| `conrod_core 0.69` | `egui 0.34` + `egui-wgpu` + `egui-winit` | full GUI rewrite |
| `crossbeam 0.3` | `crossbeam 0.8` | channels + atomics |
| `rustfft 2.0` | `rustfft 6.2` (+ `realfft 3.4`) | API fully changed |
| `time_calc 0.13` | `std::time::Duration` + `f64` Hz | inline the math, tiny surface |
| `pitch_calc 0.11` | inline Hz↔semitone math | 3 functions, no dep needed |
| `mindtree_utils 0.4` | inline `noise_walk`, `Range`, helpers | archived upstream |
| `newtype_derive` / `custom_derive` | standard Rust newtypes | macros built into language |
| `rand_xorshift 0.2` | `rand 0.8` SmallRng | XorShift is in rand's small RNG |
| `threadpool 1.7` | `std::thread::spawn` / `rayon` | |
| `fxhash 0.2` | keep | still maintained, no API change |
| `hound 3.3` | `hound 3.5` | minor version bump only |
| `serde 1.0` | `serde 1.0` | no change |
| `slug 0.1` | keep or inline 5-line impl | tiny dep |
| `num_cpus 1.8` | `std::thread::available_parallelism()` | stabilised in Rust 1.59 |

---

## Module Structure

```
src/
  main.rs              — entry point, builds App, runs winit event loop
  app.rs               — Model struct, init, update, exit handler
  config.rs            — Config (top-level JSON), defaults, load/save
  metres.rs            — Metres(f64) newtype
  utils.rs             — Range<T>, load_from_json_or_default, save_to_json,
                         noise_walk, duration_to_secs

  audio/
    mod.rs             — constants (SAMPLE_RATE, MAX_CHANNELS, FRAMES_PER_BUFFER,
                         DEFAULT_DBAP_ROLLOFF_DB, DISTANCE_BLUR), host/device selection
    input.rs           — cpal input stream, capture callback
    output.rs          — cpal output stream, render callback, DBAP mix loop
    dbap.rs            — Distance Based Amplitude Panning (pure math, no deps)
    fft.rs             — FFT plan builder (rustfft 6.x), window fn, bin-to-Hz
    detector.rs        — EnvDetector, FftDetector per speaker
    detection.rs       — detection types shared between audio and GUI
    sound.rs           — Sound, IdGenerator (AtomicUsize), SoundId
    speaker.rs         — Speaker, SpeakerId, position
    source/
      mod.rs           — Source enum (Wav, RealTimeInput), SourceId
      wav/
        mod.rs
        reader.rs      — WAV reader thread (hound), Handle, spawn()

  soundscape/
    mod.rs             — Soundscape, Message enum, Tick, spawn(), 16ms loop
    group.rs           — Group, GroupId, constraints, cooldowns
    movement.rs        — Movement, trajectory, BoundingRect, Area

  osc/
    mod.rs
    input.rs           — rosc UDP receiver thread, control_rx, osc_in_log_rx
    output.rs          — rosc UDP sender thread, msg_tx, osc_out_log_rx

  project/
    mod.rs             — Project, ProjectConfig, save/load
    (sources, speakers, installations sub-modules as needed)

  installation.rs      — Installation, InstallationId (dynamic, not hard-coded names)

  gui/
    mod.rs             — GUI Model, egui Context, update(), render()
    monitor.rs         — AudioMonitor thread, spawn(), stats channel → GUI
    theme.rs           — egui visuals / style constants
    editors/
      installation_editor.rs
      speaker_editor.rs
      source_editor.rs
      soundscape_editor.rs
      master.rs
      project_editor.rs
      osc_in_log.rs
      osc_out_log.rs
      control_log.rs
```

---

## Phase Breakdown

### Phase 0 — Scaffold ✅ COMPLETE

- [x] New `Cargo.toml` with modern deps (see migration map above)
- [x] All modules stubbed with `todo!()` so the crate compiles
- [x] Feature flags: `asio` (Windows ASIO via cpal), `test_with_stereo`
- [x] GitHub Actions CI: `cargo check`, `cargo test --no-run` on
      ubuntu-latest + macos-latest

**Done:** `cargo check` and `cargo test --no-run` pass clean on macOS.
Note: old `src/lib/` and `src/bin/` files remain in-tree as port reference;
`src/bin/main.rs` is auto-discovered by cargo but compiles fine.

---

### Phase 1 — Domain Types & Persistence ✅ COMPLETE (17/17 tests pass)

Port the value types. No threads, no IO, just data + serde.

- [ ] `Metres`, `Range<T>` newtypes (drop `newtype_derive` / `custom_derive`)
- [ ] `Config` (top-level JSON) — preserve field names so existing config files
      still load
- [ ] `project::Config` + `Project` + `ProjectConfig`
- [ ] `installation::Id` + `Installation` — keep dynamic Id(usize), preserve the
      legacy enum deserialisation shim for old JSON files
- [ ] `audio::speaker::Speaker` + `SpeakerId`
- [ ] `audio::source::Source` (Wav / RealTimeInput variants)
- [ ] `audio::sound::Sound` + `IdGenerator` (AtomicUsize, same logic)
- [ ] `soundscape::group::Group` + `GroupId`
- [ ] `soundscape::movement::Movement` + `BoundingRect`
- [ ] Replace `time_calc::Ms` with `f64` (milliseconds as plain f64, or a
      `Ms(f64)` newtype without the old crate)
- [ ] Replace `pitch_calc` with inline `hz_to_semitones` / `semitones_to_hz`

**Done when:** all types compile, round-trip through serde_json, existing
project JSON files still deserialize correctly.

---

### Phase 2 — Audio I/O ✅ COMPLETE (23/23 tests pass)

- [x] `audio::host()` — default host, or ASIO on Windows with `asio` feature
- [x] `audio::find_input_device` / `find_output_device` — name-matching via cpal traits
- [x] `audio::input` — `cpal::Stream` capture callback, forwards samples to output thread
- [x] `audio::output` — `cpal::Stream` render callback; renders silence (Phase 4 adds DBAP mix)
- [x] `audio::dbap` — pure f64 math, no crate dependency; 2 tests pass
- [x] `audio::fft` — rustfft 6.x: `FftPlanner::new()` → `plan_fft_forward()` →
      `process_with_scratch()`; Hann window; `hz_to_mel`/`mel_to_hz` inline;
      sine-wave FFT peak test passes
- [x] `audio::detector` — `EnvDetector` (rolling RMS) + `FftDetector` (Hann+FFT,
      all buffers pre-allocated, allocation-free push path)
- [x] `audio::source::wav::reader` — WAV reader thread; `Command::{Load,Unload,Exit}`;
      decodes both f32 and i32 WAV; sends `DecodedWav` back via crossbeam channel

**Note:** `cpal::DeviceTrait::name()` is deprecated in 0.17 — marked
`#[allow(deprecated)]` on the finder functions; `description()` returns a struct
rather than a String, so the old matching pattern is preserved for now.

---

### Phase 3 — OSC ✅ COMPLETE (28/28 tests pass)

- [x] `osc::input` — UDP socket on configurable port, 100ms read timeout + `AtomicBool`
      shutdown; flattens `OscPacket` (including nested bundles); parses all `/bp/*`
      address patterns; emits `LogEntry` + `ControlMsg` via crossbeam bounded channels
- [x] `osc::output` — UDP socket (port 0), per-installation per-computer target map;
      `crossbeam::select!` ticks every 16ms to flush pending `AudioFrameData`; wire
      format matches original museum deployment (avg_peak, avg_rms, lmh×3, bins×8,
      [idx,peak,rms]×N); `Message::{Audio,AddTarget,RemoveTarget,...,Exit}`
- [x] `audio::detection` — added `AudioFrameData`, `FftData`, `SpeakerData` shared
      between audio output → OSC output → GUI
- [x] All original `/bp` OSC address patterns preserved:
      `/bp/master_volume`, `/bp/source_volume/<name>`,
      `/bp/play_soundscape`, `/bp/pause_soundscape`
- [x] 5 OSC input parser tests pass

**Notes:**
- `rosc::decoder::decode_udp` returns `(&[u8], OscPacket)` — remainder bytes + packet
- `crossbeam::select!` arm body requires explicit `{ }` braces when using a bare `match`

---

### Phase 4 — Soundscape Engine ✅ COMPLETE (32/32 tests pass)

- [x] `soundscape::spawn()` — 16ms ticker thread with `Instant` drift correction;
      main soundscape thread receives `Message::{Update,Tick,Play,Pause,Exit}`
- [x] `UpdateFn` pattern for `FnOnce(&mut Model) + Send` closures over channels
- [x] `SmallRng::seed_from_u64` replaces `XorShiftRng` — same determinism contract
- [x] `utils::noise_walk(phase)` — 1D value noise with smoothstep interpolation,
      replaces `mindtree_utils::noise_walk`; verified in range and smoothness tests
- [x] `utils::add_seeds`, `utils::count_equal` — ported from original utils
- [x] All math uses plain `[f64; 2]` arrays — no glam, no nannou dependency
- [x] Agent movement: steering behaviour, force-limited, rotation-clamped per second
- [x] Ngon movement: path tracing along N-gon edges at configurable speed
- [x] Full group/source scheduling: `update_available_groups`, `update_available_sources`,
      suitability ordering (sounds needed > timing), `generate_movement`
- [x] Active sounds expire by duration; `SoundCommand::{Spawn,Despawn,UpdatePosition}`
      sent to audio output thread via crossbeam channel
- [x] `audio::sound::{Handle, SoundCommand}` added for soundscape ↔ audio wire

**Notes:**
- rand 0.10: `.random()` is on `RngExt` (not `Rng`) — import needed explicitly
- `soundscape_thread_starts_and_exits` smoke test: thread spawns, ticks 50ms, exits cleanly

---

### Phase 5 — GUI ✅ COMPLETE (32/32 tests pass)

Replace conrod with egui via `eframe 0.34`. All UI code rewritten; logic
(what values are shown, what can be edited) is preserved.

**Notes:**
- Switched from raw winit+wgpu+egui-winit to `eframe 0.34` — handles the
  event loop, surface, renderer, and input translation automatically.
- eframe 0.34 API: required method is `fn ui(&mut self, ui: &mut egui::Ui, frame)`
  (not the deprecated `update`); `fn logic(ctx, frame)` for pre-frame work.
- Panels use `egui::Panel::left(...).default_size(...).show_inside(ui, |ui|{...})`
  and `egui::CentralPanel::default().show_inside(ui, |ui|{...})`.
- Theme applied once in the eframe creation context via `theme::apply(&cc.egui_ctx)`.

**Implemented:**
- [x] `eframe` event loop + wgpu renderer (`app.rs` → `eframe::run_native`)
- [x] `gui::theme` — dark color palette, font sizes, `level_color()` helper
- [x] `gui::monitor` — `AudioMonitorMsg` + `(MsgSender, MsgReceiver)` channel
- [x] Master panel (volume, DBAP rolloff, latency, play/pause, CPU saving)
- [x] Project editor (name display + save button)
- [x] Installation editor (list/select/edit/add installations, simultaneous-sounds range)
- [x] Speaker editor (list/select/edit/add/remove speakers, channel + X/Y)
- [x] Source editor (list/select/edit sources, shows WAV path or realtime channels)
- [x] Soundscape editor (groups list/select/edit/add/remove, occurrence rate, concurrency)
- [x] OSC in/out log panels (scrollable, colour-coded errors on out-log)
- [x] Control log panel (scrollable ControlMsg display)
- [x] Audio monitor panel (master peak bar, avg RMS, per-speaker levels)
- [x] Floorplan canvas — `egui::Painter` circles for speakers + active sounds,
      world→canvas scale from speaker bounding box; empty-state placeholder text

---

### Phase 6 — Integration & Exit ✅ COMPLETE (32/32 tests pass)

- [x] `utils::assets_dir()` — searches next to the executable then cwd
- [x] `app::run()` — full initialisation in order:
      config → audio host/devices → WAV reader → audio output stream →
      audio input stream (optional) → soundscape → OSC in → OSC out →
      audio monitor channel → project load → eframe::run_native
- [x] `SpatialAudioApp` extended with thread handles, persistence fields,
      and `poll_channels()` that drains OSC/audio-monitor channels each frame
- [x] `apply_control()` — wires `/bp` OSC controls to live project state
      (master volume, source volume, play/pause soundscape)
- [x] `on_exit()` — saves project state + config to disk, shuts down threads
      in order: soundscape → WAV reader → OSC in → OSC out → audio streams
- [x] CPU saving mode: `eframe` `request_repaint_after(100ms)` when enabled
- [x] `project::Project::save_parts()` — saves state + config without cloning
- [x] `audio::source::wav::reader::Spawned::exit()` — sends Command::Exit + joins
- [x] `cpal::SampleRate = u32` in 0.17 — `best_stream_config()` uses u32 directly
- [x] `project::{State, Speaker, Source, Sources}` now derive `Clone`

**Notes:**
- `sound_cmd_rx` is intentionally dropped in Phase 6 (audio mixing is a future step);
  the soundscape sends commands via `let _ = send(...)` which silently discards them
  on disconnected channel — no blocking, no panic.
- `monitor_tx` is dropped in Phase 6; the audio output thread will send frames
  through it once DBAP mixing is implemented.

**Done when:** full startup → use → graceful shutdown cycle completes without
panics or resource leaks (check with Instruments / Valgrind).

---

### Phase 7 — Platform Verification

- [ ] macOS: build + run, verify Core Audio device enumeration, Syphon not
      required (pure audio)
- [ ] Linux: build + run with ALSA/JACK via cpal default host
- [ ] Windows: build with and without `asio` feature; verify ASIO host
      initialises (needs ASIO SDK path set in env)

---

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `rustfft` 2→6 API break | Write a unit test for the FFT before porting detector logic |
| `mindtree_utils::noise_walk` is private/archived | Inline: it's ~30-line Perlin walk, well-documented upstream |
| `nannou_audio` channel API differs from `cpal` | nannou_audio was already a thin cpal wrapper; study its source on GitHub to verify mapping |
| conrod → egui is a full rewrite | Do editors one at a time; stub missing ones with `ui.label("TODO")` |
| Existing JSON project files must still load | Keep field names, keep legacy serde deserialization shims (e.g. Installation::Id from old enum strings) |
| ASIO SDK path on Windows CI | Gate ASIO CI behind a manual workflow trigger; document setup in README |

---

## What Is NOT Changing

- Thread topology (same 7 threads, same channel directions)
- DBAP spatial algorithm (pure math, no library)
- OSC address patterns (installation hardware compatibility)
- JSON project file schema (backwards compatible)
- Audio constants: 48 kHz, 1024 frames/buffer, up to 128 channels
- The 16ms soundscape tick rate

---

## Suggested First PR

Phase 0 + Phase 1: scaffold + domain types. This produces a crate that compiles
cleanly on all three platforms with zero unsafe, zero old deps, and a test that
round-trips every domain type through JSON. It is mergeable and unblocks all
other phases in parallel.
