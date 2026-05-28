pub mod reader;

use crate::audio::SAMPLE_RATE;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A pre-rendered WAV file audio source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wav {
    pub path: PathBuf,
    pub channels: usize,
    /// Duration in samples (serialised as i64 to match original time_calc::Samples).
    pub duration: i64,
    /// Sample rate in Hz (serialised as f64 to match original time_calc::SampleHz).
    pub sample_hz: f64,
    #[serde(default = "default_should_loop")]
    pub should_loop: bool,
    #[serde(default = "default_playback")]
    pub playback: Playback,
}

impl Wav {
    pub fn from_path(path: PathBuf) -> Result<Self, hound::Error> {
        let reader = hound::WavReader::open(&path)?;
        let spec = reader.spec();
        let channels = spec.channels as usize;
        let sample_hz = spec.sample_rate as f64;
        assert_eq!(
            sample_hz, SAMPLE_RATE,
            "WAV files must have a sample rate of {SAMPLE_RATE}"
        );
        let duration = reader.duration() as i64;
        Ok(Wav {
            path,
            channels,
            duration,
            sample_hz,
            should_loop: default_should_loop(),
            playback: default_playback(),
        })
    }

    pub fn duration_ms(&self) -> crate::utils::Ms {
        crate::utils::Ms(self.duration as f64 / self.sample_hz * 1_000.0)
    }
}

/// Playback mode for a WAV source.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Playback {
    /// Start from the beginning each time the sound is triggered.
    Retrigger,
    /// Play as though the WAV is continuously looping; the sound is just "unmuted" when triggered.
    Continuous,
}

pub const NUM_PLAYBACK_OPTIONS: usize = 2;

fn default_playback() -> Playback { Playback::Retrigger }
fn default_should_loop() -> bool { false }
