pub mod dbap;
pub mod detection;
pub mod detector;
pub mod fft;
pub mod input;
pub mod output;
pub mod sound;
pub mod source;
pub mod speaker;

use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(not(feature = "test_with_stereo"))]
pub const MAX_CHANNELS: usize = 128;
#[cfg(feature = "test_with_stereo")]
pub const MAX_CHANNELS: usize = 2;

pub const MAX_SOUNDS: usize = 1024;
pub const SAMPLE_RATE: f64 = 48_000.0;
pub const FRAMES_PER_BUFFER: usize = 1024;
pub const DEFAULT_MASTER_VOLUME: f32 = 0.5;
pub const DEFAULT_DBAP_ROLLOFF_DB: f64 = 4.0;
pub const DISTANCE_BLUR: f64 = 0.01;
pub const DEFAULT_REALTIME_SOURCE_LATENCY_MS: f64 = 512.0;
pub const DEFAULT_PROXIMITY_LIMIT_METRES: f64 = 7.0;
pub const DEFAULT_PROXIMITY_LIMIT_METRES_2: f64 =
    DEFAULT_PROXIMITY_LIMIT_METRES * DEFAULT_PROXIMITY_LIMIT_METRES;

/// Select the audio host — ASIO on Windows when the `asio` feature is enabled,
/// falling back to the platform default.
pub fn host() -> cpal::Host {
    #[cfg(all(windows, feature = "asio"))]
    {
        if let Ok(h) = cpal::host_from_id(cpal::HostId::Asio) {
            return h;
        }
    }
    cpal::default_host()
}

/// Find the first output device whose name contains `target` (case-insensitive).
/// Returns the host default when `target` is empty or no match is found.
#[allow(deprecated)]
pub fn find_output_device(host: &cpal::Host, target: &str) -> Option<cpal::Device> {
    if target.is_empty() {
        return host.default_output_device();
    }
    let lower = target.to_lowercase();
    host.output_devices()
        .ok()?
        .find(|d| d.name().ok().map(|n| n.to_lowercase().contains(&lower)).unwrap_or(false))
        .or_else(|| host.default_output_device())
}

/// Find the first input device whose name contains `target` (case-insensitive).
/// Returns the host default when `target` is empty or no match is found.
#[allow(deprecated)]
pub fn find_input_device(host: &cpal::Host, target: &str) -> Option<cpal::Device> {
    if target.is_empty() {
        return host.default_input_device();
    }
    let lower = target.to_lowercase();
    host.input_devices()
        .ok()?
        .find(|d| d.name().ok().map(|n| n.to_lowercase().contains(&lower)).unwrap_or(false))
        .or_else(|| host.default_input_device())
}
