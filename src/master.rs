use crate::audio;
use crate::metres::Metres;
use crate::utils::Ms;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Master {
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default = "default_realtime_source_latency")]
    pub realtime_source_latency: Ms,
    #[serde(default = "default_dbap_rolloff_db")]
    pub dbap_rolloff_db: f64,
    /// Proximity limit stored squared for faster DBAP calculations.
    #[serde(default = "default_proximity_limit_2")]
    pub proximity_limit_2: Metres,
}

fn default_volume() -> f32 { audio::DEFAULT_MASTER_VOLUME }
fn default_realtime_source_latency() -> Ms { Ms(audio::DEFAULT_REALTIME_SOURCE_LATENCY_MS) }
fn default_dbap_rolloff_db() -> f64 { audio::DEFAULT_DBAP_ROLLOFF_DB }
fn default_proximity_limit_2() -> Metres { Metres(audio::DEFAULT_PROXIMITY_LIMIT_METRES_2) }

impl Default for Master {
    fn default() -> Self {
        Master {
            volume: default_volume(),
            realtime_source_latency: default_realtime_source_latency(),
            dbap_rolloff_db: default_dbap_rolloff_db(),
            proximity_limit_2: default_proximity_limit_2(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_round_trip() {
        let m = Master::default();
        let json = serde_json::to_string(&m).unwrap();
        let back: Master = serde_json::from_str(&json).unwrap();
        assert_eq!(m.volume, back.volume);
    }
}
