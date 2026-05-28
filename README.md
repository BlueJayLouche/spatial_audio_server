# Spatial Audio Server

A cross-platform, n-channel spatial audio server developed by
[MindBuffer](https://www.mindbuffer.net/). This software was commissioned by
Museums Victoria to create the soundscape of *Beyond Perception: Seeing the
Unseen*, a permanent exhibition at Scienceworks in Melbourne, Australia which
opened in May 2018.

This repository contains a full rewrite of the original server, replacing the
unmaintained nannou 0.13 / conrod stack with a modern, maintained toolchain
while preserving all original features and the proven 7-thread architecture.

## Features

- Stores and plays back multi-channel WAV content.
- Interfaces with the system's audio input and output devices via **cpal**
  (Core Audio on macOS, ALSA/JACK on Linux, WASAPI/ASIO on Windows).
- Analyses audio data per-speaker and sends it to installation computers via
  **OSC** (rosc).
- Responds to control values via OSC (`/bp/master_volume`,
  `/bp/source_volume/<name>`, `/bp/play_soundscape`, `/bp/pause_soundscape`).
- Generatively produces a spatial soundscape using a 16 ms tick engine with
  Agent steering and N-gon path-tracing movement.
- **DBAP** (Distance Based Amplitude Panning) spatial mixing.
- GUI built with **egui** via **eframe**: collapsible side-panel editors,
  live audio monitor, and a 2-D floorplan canvas.

## Building

### Requirements

- [Rust](https://www.rust-lang.org/) stable (2021 edition or later)
- On **Windows** with ASIO: set `CPAL_ASIO_DIR` to your ASIO SDK path and
  build with `--features asio`.

### Quick start

```sh
cargo run
```

The binary looks for an `assets/` directory next to the executable (or in the
current working directory when running via `cargo run` from the repo root).
Project files are stored under `assets/projects/<slug>/`.

### Platforms

| Platform | Backend | Status |
|---|---|---|
| macOS | Core Audio (Metal via wgpu) | ✅ primary |
| Linux | ALSA / JACK | ✅ supported |
| Windows | WASAPI | ✅ supported |
| Windows ASIO | ASIO SDK | ✅ `--features asio` |

### Feature flags

| Flag | Description |
|---|---|
| `asio` | Enable ASIO audio host on Windows (requires ASIO SDK) |
| `test_with_stereo` | Limit to 2 audio channels for CI/unit tests |

## Architecture

Seven threads communicate via crossbeam channels:

```
main / GUI (eframe)
  ├── OSC in  (rosc UDP receiver)
  ├── OSC out (rosc UDP sender, ~60 fps)
  ├── Soundscape (16 ms tick, Agent / Ngon movement)
  │     └── WAV reader (hound decoder)
  ├── Audio output (cpal, DBAP mixing)
  └── Audio input  (cpal, real-time capture)
```

The thread topology and all audio constants (48 kHz, 1024 frames/buffer, up
to 128 channels, 16 ms soundscape tick) are identical to the original
museum deployment.

## Dependency migration

| Original | Replacement |
|---|---|
| `nannou 0.13` + `conrod` | `eframe 0.34` (egui + wgpu + winit) |
| `nannou_audio` | `cpal 0.17` |
| `nannou_osc` | `rosc 0.11` |
| `rustfft 2` | `rustfft 6` + `realfft 3` |
| `time_calc` / `pitch_calc` | inline math |
| `mindtree_utils` | inlined `noise_walk`, `Range` |
| `newtype_derive` / `custom_derive` | standard Rust newtypes |
| `rand_xorshift` | `rand 0.10` `SmallRng` |
| `crossbeam 0.3` | `crossbeam 0.8` |
| `num_cpus` | `std::thread::available_parallelism()` |

## OSC protocol

All addresses are prefixed with `/bp`:

| Address | Args | Direction |
|---|---|---|
| `/bp/master_volume` | `Float 0.0–1.0` | in |
| `/bp/source_volume/<name>` | `Float` | in |
| `/bp/play_soundscape` | — | in |
| `/bp/pause_soundscape` | — | in |
| `/<installation>` | avg_peak, avg_rms, lmh×3, bins×8, [idx,peak,rms]×N | out |

## Project files

Projects are stored as JSON under `assets/projects/<slug>/`:

- `state.json` — speakers, sources, installations, soundscape groups, master
- `config.json` — OSC port, window size, seed, log limits

Existing project files from the original 2018 deployment load without
modification (legacy installation ID strings are shimmed during deserialisation).

## Original work

The original server was developed by Mitchell Nordine / MindBuffer for Museums
Victoria. This rewrite preserves the DBAP algorithm, OSC wire format, and
project JSON schema for full backwards compatibility with the museum hardware.
