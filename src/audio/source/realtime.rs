use crate::utils::Ms;
use serde::{Deserialize, Serialize};
use std::ops;

/// A real-time audio input source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Realtime {
    pub duration: Ms,
    pub channels: ops::Range<usize>,
}

impl Default for Realtime {
    fn default() -> Self {
        Realtime {
            duration: Ms(crate::audio::DEFAULT_REALTIME_SOURCE_LATENCY_MS),
            channels: 0..2,
        }
    }
}
