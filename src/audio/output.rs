use crate::audio::{self, dbap, sound};
use crate::audio::source::wav::reader::DecodedWav;
use crossbeam::channel::Receiver;
use cpal::traits::{DeviceTrait, StreamTrait};
use fxhash::FxHashMap;

// ── Per-sound source kind (output-thread copy) ────────────────────────────────

enum ActiveSourceKind {
    Wav { wav_id: u64 },
    Realtime { channels: std::ops::Range<usize> },
}

// ── Active sound state ────────────────────────────────────────────────────────

struct ActiveSoundState {
    kind: ActiveSourceKind,
    position: sound::Position,
    attack_frames: i64,
    release_frames: i64,
    duration_frames: Option<i64>,
    elapsed_frames: i64,
    /// Frame index into the decoded WAV (loops). Unused for Realtime sources.
    sample_pos: usize,
}

// ── Cached decoded WAV ────────────────────────────────────────────────────────

struct CachedWav {
    samples: Vec<f32>,
    channels: usize,
}

// ── Output engine (lives inside the cpal callback closure) ────────────────────

struct OutputState {
    channels: usize,
    master_volume: f32,
    rolloff_db: f64,

    sound_cmd_rx: Receiver<sound::SoundCommand>,
    wav_rx: Receiver<DecodedWav>,
    input_rx: Receiver<Vec<f32>>,

    active_sounds: FxHashMap<sound::Id, ActiveSoundState>,
    wav_cache: FxHashMap<u64, CachedWav>,
    /// Circular buffer of interleaved f32 samples from the live input stream.
    input_buf: Vec<f32>,
    input_channels: usize,

    speakers: Vec<sound::SpeakerSnapshot>,
    /// DBAP scratch — one entry per speaker, resized only when SetSpeakers arrives.
    dbap_scratch: Vec<dbap::Speaker>,
    /// Gain scratch — one f32 per speaker, resized only when SetSpeakers arrives.
    gain_scratch: Vec<f32>,
    /// Reused per-render to collect expired ids without allocating.
    expired: Vec<sound::Id>,
}

impl OutputState {
    fn drain_commands(&mut self) {
        while let Ok(cmd) = self.sound_cmd_rx.try_recv() {
            match cmd {
                sound::SoundCommand::Spawn { id, kind, position, attack_frames, release_frames, duration_frames, .. } => {
                    let active_kind = match kind {
                        sound::AudioSourceKind::Wav { id: wav_id } => ActiveSourceKind::Wav { wav_id },
                        sound::AudioSourceKind::Realtime { channels } => ActiveSourceKind::Realtime { channels },
                    };
                    self.active_sounds.insert(id, ActiveSoundState {
                        kind: active_kind,
                        position,
                        attack_frames,
                        release_frames,
                        duration_frames,
                        elapsed_frames: 0,
                        sample_pos: 0,
                    });
                }
                sound::SoundCommand::Despawn(id) => {
                    self.active_sounds.remove(&id);
                }
                sound::SoundCommand::UpdatePosition { id, position } => {
                    if let Some(s) = self.active_sounds.get_mut(&id) {
                        s.position = position;
                    }
                }
                sound::SoundCommand::SetSpeakers(spks) => {
                    let n = spks.len();
                    self.speakers = spks;
                    self.dbap_scratch.resize(n, dbap::Speaker { distance_sq: 1.0, weight: 0.0 });
                    self.gain_scratch.resize(n, 0.0);
                }
            }
        }

        while let Ok(decoded) = self.wav_rx.try_recv() {
            self.wav_cache.insert(decoded.id, CachedWav {
                samples: decoded.samples,
                channels: decoded.channels,
            });
        }

        // Accumulate live input; keep at most ~1 s to bound memory.
        while let Ok(chunk) = self.input_rx.try_recv() {
            self.input_buf.extend_from_slice(&chunk);
        }
        let max_samples = audio::SAMPLE_RATE as usize * self.input_channels.max(1);
        if self.input_buf.len() > max_samples {
            let excess = self.input_buf.len() - max_samples;
            self.input_buf.drain(..excess);
        }
    }

    fn render(&mut self, data: &mut [f32]) {
        self.drain_commands();
        data.fill(0.0);

        let n_speakers = self.speakers.len();
        if n_speakers == 0 || self.active_sounds.is_empty() {
            return;
        }

        let n_frames = data.len() / self.channels;

        // Copy scalar fields before the split borrow so we don't need to
        // dereference through &mut pointers inside the hot loop.
        let n_ch = self.channels;
        let mv = self.master_volume;
        let rolloff = self.rolloff_db;
        let in_ch = self.input_channels;

        // Split-borrow: extract mutable refs to each field independently so the
        // borrow checker lets us access them simultaneously inside the loop.
        let Self {
            active_sounds,
            wav_cache,
            input_buf,
            speakers,
            dbap_scratch,
            gain_scratch,
            expired,
            ..
        } = self;

        expired.clear();

        for (&sound_id, sound_state) in active_sounds.iter_mut() {
            // Expire sounds that have run their full duration.
            if let Some(dur) = sound_state.duration_frames {
                if sound_state.elapsed_frames >= dur {
                    expired.push(sound_id);
                    continue;
                }
            }

            // Compute DBAP gains once per render block (position changes only
            // on UpdatePosition, not per-sample).
            let src_pt = [sound_state.position.point.x.0, sound_state.position.point.y.0];
            for i in 0..n_speakers {
                dbap_scratch[i] = dbap::Speaker {
                    distance_sq: dbap::blurred_distance_2(src_pt, speakers[i].point, audio::DISTANCE_BLUR),
                    weight: 1.0,
                };
            }
            {
                let mut gi = dbap::SpeakerGains::new(&dbap_scratch[..n_speakers], rolloff);
                for g in gain_scratch.iter_mut().take(n_speakers) {
                    *g = gi.next().unwrap_or(0.0) as f32;
                }
            }

            // Mix frame by frame.
            for frame_idx in 0..n_frames {
                let env = envelope_gain(
                    sound_state.elapsed_frames,
                    sound_state.attack_frames,
                    sound_state.release_frames,
                    sound_state.duration_frames,
                );

                let raw = match &sound_state.kind {
                    ActiveSourceKind::Wav { wav_id } => {
                        wav_sample(*wav_id, sound_state.sample_pos, wav_cache)
                    }
                    ActiveSourceKind::Realtime { channels: rt_ch } => {
                        realtime_sample(rt_ch, input_buf, in_ch)
                    }
                };

                let s = raw * env * mv;
                let frame_start = frame_idx * n_ch;
                for i in 0..n_speakers {
                    let ch = speakers[i].channel;
                    if ch < n_ch {
                        data[frame_start + ch] += s * gain_scratch[i];
                    }
                }

                if matches!(sound_state.kind, ActiveSourceKind::Wav { .. }) {
                    sound_state.sample_pos += 1;
                }
                sound_state.elapsed_frames += 1;
            }
        }

        for id in expired.iter() {
            active_sounds.remove(id);
        }
    }
}

// ── Sample helpers (allocation-free) ─────────────────────────────────────────

/// Read one mono sample from a cached WAV at `frame_pos`, looping.
fn wav_sample(wav_id: u64, frame_pos: usize, cache: &FxHashMap<u64, CachedWav>) -> f32 {
    let cached = match cache.get(&wav_id) { Some(c) => c, None => return 0.0 };
    let ch = cached.channels.max(1);
    let n_frames = cached.samples.len() / ch;
    if n_frames == 0 { return 0.0; }
    let frame = frame_pos % n_frames;
    let base = frame * ch;
    let mut s = 0.0f32;
    for c in 0..ch {
        s += cached.samples.get(base + c).copied().unwrap_or(0.0);
    }
    s / ch as f32
}

/// Mix down the selected input channels from the tail of `input_buf` (live audio).
fn realtime_sample(rt_channels: &std::ops::Range<usize>, input_buf: &[f32], input_ch: usize) -> f32 {
    if input_buf.is_empty() || input_ch == 0 { return 0.0; }
    let n_frames = input_buf.len() / input_ch;
    if n_frames == 0 { return 0.0; }
    let tail = (n_frames - 1) * input_ch;
    let mut s = 0.0f32;
    let mut n = 0usize;
    for ch in rt_channels.clone() {
        if ch < input_ch {
            s += input_buf[tail + ch];
            n += 1;
        }
    }
    if n > 0 { s / n as f32 } else { 0.0 }
}

/// Linear attack/release envelope — returns a gain in [0.0, 1.0].
fn envelope_gain(elapsed: i64, attack: i64, release: i64, duration: Option<i64>) -> f32 {
    let attack_gain = if attack > 0 {
        (elapsed as f32 / attack as f32).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let release_gain = if let Some(dur) = duration {
        if release > 0 {
            let remaining = dur - elapsed;
            if remaining < release {
                (remaining as f32 / release as f32).max(0.0)
            } else {
                1.0
            }
        } else {
            1.0
        }
    } else {
        1.0
    };
    attack_gain.min(release_gain)
}

// ── Public model ──────────────────────────────────────────────────────────────

/// A live cpal output stream with DBAP spatial mixing.
pub struct Model {
    _stream: cpal::Stream,
}

impl Model {
    /// Build and start a cpal output stream.
    ///
    /// - `sound_cmd_rx` — receives Spawn/Despawn/UpdatePosition/SetSpeakers from the soundscape and GUI.
    /// - `wav_rx` — receives decoded WAV data from the WAV reader thread.
    /// - `input_rx` — receives interleaved f32 chunks from the audio input callback.
    /// - `initial_speakers` — DBAP speaker list at startup; updated via `SetSpeakers` later.
    /// - `input_channels` — channel count of the input stream (used for channel selection).
    pub fn new(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        sound_cmd_rx: crossbeam::channel::Receiver<sound::SoundCommand>,
        wav_rx: crossbeam::channel::Receiver<DecodedWav>,
        input_rx: crossbeam::channel::Receiver<Vec<f32>>,
        master_volume: f32,
        rolloff_db: f64,
        initial_speakers: Vec<sound::SpeakerSnapshot>,
        input_channels: usize,
    ) -> anyhow::Result<Self> {
        let channels = config.channels as usize;
        let n_spk = initial_speakers.len();

        let mut state = OutputState {
            channels,
            master_volume,
            rolloff_db,
            sound_cmd_rx,
            wav_rx,
            input_rx,
            active_sounds: FxHashMap::with_capacity_and_hasher(
                audio::MAX_SOUNDS,
                Default::default(),
            ),
            wav_cache: FxHashMap::default(),
            input_buf: Vec::new(),
            input_channels,
            speakers: initial_speakers,
            dbap_scratch: vec![dbap::Speaker { distance_sq: 1.0, weight: 0.0 }; n_spk],
            gain_scratch: vec![0.0f32; n_spk],
            expired: Vec::new(),
        };

        let err_fn = |e| eprintln!("output stream error: {e}");
        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                state.render(data);
            },
            err_fn,
            None,
        )?;
        stream.play()?;
        Ok(Model { _stream: stream })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_full_sustain() {
        // No attack, no release, no duration limit → always 1.0
        assert_eq!(envelope_gain(0, 0, 0, None), 1.0);
        assert_eq!(envelope_gain(1000, 0, 0, None), 1.0);
    }

    #[test]
    fn envelope_attack_ramp() {
        // 100-frame attack: at frame 50 gain should be 0.5
        let g = envelope_gain(50, 100, 0, None);
        assert!((g - 0.5).abs() < 1e-5, "expected 0.5 got {g}");
    }

    #[test]
    fn envelope_release_ramp() {
        // 200-frame duration, 100-frame release: at frame 150 (50 frames from end) gain = 0.5
        let g = envelope_gain(150, 0, 100, Some(200));
        assert!((g - 0.5).abs() < 1e-5, "expected 0.5 got {g}");
    }

    #[test]
    fn wav_sample_loops() {
        let mut cache = FxHashMap::default();
        cache.insert(0u64, CachedWav { samples: vec![1.0, 0.0], channels: 1 });
        assert_eq!(wav_sample(0, 0, &cache), 1.0);
        assert_eq!(wav_sample(0, 1, &cache), 0.0);
        assert_eq!(wav_sample(0, 2, &cache), 1.0); // loops
    }

    #[test]
    fn wav_sample_mono_mixdown() {
        let mut cache = FxHashMap::default();
        // Stereo: L=1.0, R=0.5 → mono = 0.75
        cache.insert(1u64, CachedWav { samples: vec![1.0, 0.5], channels: 2 });
        let s = wav_sample(1, 0, &cache);
        assert!((s - 0.75).abs() < 1e-5, "expected 0.75 got {s}");
    }
}
