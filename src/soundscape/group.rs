use crate::utils::{Ms, Range, HR_MS};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Id(pub usize);

/// Soundscape group — describes occurrence and concurrency constraints shared across
/// multiple sources.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    #[serde(default = "default::occurrence_rate")]
    pub occurrence_rate: Range<Ms>,
    #[serde(default = "default::simultaneous_sounds")]
    pub simultaneous_sounds: Range<usize>,
}

pub mod default {
    use super::*;

    pub const OCCURRENCE_RATE: Range<Ms> = Range { min: Ms(0.0), max: Ms(HR_MS) };
    pub const SIMULTANEOUS_SOUNDS: Range<usize> = Range { min: 1, max: 10 };

    pub fn occurrence_rate() -> Range<Ms> { OCCURRENCE_RATE }
    pub fn simultaneous_sounds() -> Range<usize> { SIMULTANEOUS_SOUNDS }
}

impl Default for Group {
    fn default() -> Self {
        Group {
            occurrence_rate: default::OCCURRENCE_RATE,
            simultaneous_sounds: default::SIMULTANEOUS_SOUNDS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_round_trip() {
        let g = Group::default();
        let json = serde_json::to_string(&g).unwrap();
        let back: Group = serde_json::from_str(&json).unwrap();
        assert_eq!(g, back);
    }
}
