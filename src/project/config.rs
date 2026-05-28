use crate::audio;
use crate::metres::Metres;
use crate::utils::Seed;
use serde::{Deserialize, Serialize};

/// Per-project configuration — persisted to `<project>/config.json`.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default::window_width")]
    pub window_width: u32,
    #[serde(default = "default::window_height")]
    pub window_height: u32,
    #[serde(default = "default::osc_input_port")]
    pub osc_input_port: u16,
    #[serde(default = "default::osc_input_log_limit")]
    pub osc_input_log_limit: usize,
    #[serde(default = "default::osc_output_log_limit")]
    pub osc_output_log_limit: usize,
    #[serde(default = "default::control_log_limit")]
    pub control_log_limit: usize,
    #[serde(default = "default::floorplan_pixels_per_metre")]
    pub floorplan_pixels_per_metre: f64,
    #[serde(default = "default::min_speaker_radius_metres")]
    pub min_speaker_radius_metres: Metres,
    #[serde(default = "default::max_speaker_radius_metres")]
    pub max_speaker_radius_metres: Metres,
    #[serde(default = "default::seed")]
    pub seed: Seed,
    /// Stored squared for faster DBAP calculations.
    #[serde(default = "default::proximity_limit_2")]
    pub proximity_limit_2: Metres,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            window_width: default::window_width(),
            window_height: default::window_height(),
            osc_input_port: default::osc_input_port(),
            osc_input_log_limit: default::osc_input_log_limit(),
            osc_output_log_limit: default::osc_output_log_limit(),
            control_log_limit: default::control_log_limit(),
            floorplan_pixels_per_metre: default::floorplan_pixels_per_metre(),
            min_speaker_radius_metres: default::min_speaker_radius_metres(),
            max_speaker_radius_metres: default::max_speaker_radius_metres(),
            seed: default::seed(),
            proximity_limit_2: default::proximity_limit_2(),
        }
    }
}

pub mod default {
    use crate::audio;
    use crate::metres::Metres;
    use crate::utils::Seed;

    pub fn window_width() -> u32 { 1280 }
    pub fn window_height() -> u32 { 720 }
    pub fn osc_input_port() -> u16 { 9001 }
    pub fn osc_input_log_limit() -> usize { 50 }
    pub fn osc_output_log_limit() -> usize { 10 }
    pub fn control_log_limit() -> usize { 50 }
    pub fn floorplan_pixels_per_metre() -> f64 { 148.0 }
    pub fn min_speaker_radius_metres() -> Metres { Metres(0.25) }
    pub fn max_speaker_radius_metres() -> Metres { Metres(1.0) }
    pub fn seed() -> Seed { [0; 16] }
    pub fn proximity_limit_2() -> Metres { Metres(audio::DEFAULT_PROXIMITY_LIMIT_METRES_2) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let c = Config::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
