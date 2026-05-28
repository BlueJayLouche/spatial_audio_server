pub mod realtime;
pub mod wav;

use crate::geom::Point2;
use crate::installation;
use crate::soundscape::group;
use crate::utils::{Ms, Range, HR_MS, MIN_MS};
use fxhash::FxHashSet;
use serde::{Deserialize, Serialize};


/// Source Ids use `u64` to match the original format.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Id(pub u64);

// ── Source ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub kind: Kind,
    #[serde(default)]
    pub role: Option<Role>,
    #[serde(default = "default::spread")]
    pub spread: crate::metres::Metres,
    #[serde(default = "default::channel_radians")]
    pub channel_radians: f32,
    #[serde(default = "default::volume")]
    pub volume: f32,
    #[serde(default)]
    pub muted: bool,
}

impl Source {
    pub fn channel_count(&self) -> usize {
        match &self.kind {
            Kind::Wav(w) => w.channels,
            Kind::Realtime(rt) => rt.channels.end - rt.channels.start,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Kind {
    Wav(wav::Wav),
    Realtime(realtime::Realtime),
}

// ── Role ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Role {
    Soundscape(Soundscape),
    Interactive,
    Scribbles,
}

impl Role {
    pub fn soundscape_mut(&mut self) -> Option<&mut Soundscape> {
        match self {
            Role::Soundscape(s) => Some(s),
            _ => None,
        }
    }
}

// ── Soundscape role params ────────────────────────────────────────────────────

pub const MAX_PLAYBACK_DURATION: Ms = Ms(HR_MS * 24.0);
pub const MAX_ATTACK_DURATION: Ms = Ms(MIN_MS);
pub const MAX_RELEASE_DURATION: Ms = Ms(MIN_MS);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Soundscape {
    #[serde(default)]
    pub installations: FxHashSet<installation::Id>,
    #[serde(default)]
    pub groups: FxHashSet<group::Id>,
    #[serde(default = "default::occurrence_rate")]
    pub occurrence_rate: Range<Ms>,
    #[serde(default = "default::simultaneous_sounds")]
    pub simultaneous_sounds: Range<usize>,
    #[serde(default = "default::playback_duration")]
    pub playback_duration: Range<Ms>,
    #[serde(default = "default::attack_duration")]
    pub attack_duration: Range<Ms>,
    #[serde(default = "default::release_duration")]
    pub release_duration: Range<Ms>,
    #[serde(default = "default::movement")]
    pub movement: Movement,
}

impl Default for Soundscape {
    fn default() -> Self {
        Soundscape {
            installations: Default::default(),
            groups: Default::default(),
            occurrence_rate: default::OCCURRENCE_RATE,
            simultaneous_sounds: default::SIMULTANEOUS_SOUNDS,
            playback_duration: default::PLAYBACK_DURATION,
            attack_duration: default::ATTACK_DURATION,
            release_duration: default::RELEASE_DURATION,
            movement: default::MOVEMENT,
        }
    }
}

// ── Movement (source config) ──────────────────────────────────────────────────

/// How the source's sounds move within the installation — stored in the project file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Movement {
    /// Fixed normalised position (0.0–1.0) within the installation area.
    Fixed(Point2<f64>),
    Generative(Generative),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Generative {
    Agent(Agent),
    Ngon(Ngon),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    #[serde(default = "default::max_speed")]
    pub max_speed: Range<f64>,
    #[serde(default = "default::max_force")]
    pub max_force: Range<f64>,
    #[serde(default = "default::max_rotation")]
    pub max_rotation: Range<f64>,
    #[serde(default = "default::directional")]
    pub directional: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ngon {
    pub vertices: Range<usize>,
    pub nth: Range<usize>,
    /// Normalised (0.0–1.0) dimensions of the polygon; serialised as `{"x": ..., "y": ...}`
    /// to match the original `nannou::geom::Vector2` format.
    pub normalised_dimensions: Point2<f64>,
    #[serde(default = "default::radians_offset")]
    pub radians_offset: Range<f64>,
    pub speed: Range<f64>,
}

impl Movement {
    pub const VARIANT_COUNT: usize = 2;
    pub fn to_index(&self) -> usize {
        match self { Movement::Fixed(_) => 0, Movement::Generative(_) => 1 }
    }
}

pub mod movement_constants {
    pub const MAX_SPEED: f64 = 20.0;
    pub const MAX_FORCE: f64 = 1.0;
    pub const MAX_ROTATION: f64 = 100.0 * std::f64::consts::PI;
    pub const MAX_VERTICES: usize = 50;
    pub const MAX_RADIANS_OFFSET: f64 = 2.0 * std::f64::consts::PI;
}

// ── Defaults ──────────────────────────────────────────────────────────────────

pub mod default {
    use super::*;
    use crate::metres::Metres;

    pub const SPREAD: Metres = Metres(2.5);
    pub const CHANNEL_RADIANS: f32 = std::f32::consts::PI * 0.5;
    pub const VOLUME: f32 = 0.6;
    pub const OCCURRENCE_RATE: Range<Ms> = Range { min: Ms(500.0), max: Ms(HR_MS) };
    pub const SIMULTANEOUS_SOUNDS: Range<usize> = Range { min: 0, max: 1 };
    pub const PLAYBACK_DURATION: Range<Ms> = Range {
        min: MAX_PLAYBACK_DURATION,
        max: MAX_PLAYBACK_DURATION,
    };
    pub const ATTACK_DURATION: Range<Ms> = Range { min: Ms(0.0), max: Ms(0.0) };
    pub const RELEASE_DURATION: Range<Ms> = Range { min: Ms(0.0), max: Ms(0.0) };
    pub const FIXED: Point2<f64> = Point2 { x: 0.5, y: 0.5 };
    pub const MAX_SPEED: Range<f64> = Range { min: 1.0, max: 5.0 };
    pub const MAX_FORCE: Range<f64> = Range { min: 0.04, max: 0.06 };
    pub const MAX_ROTATION: Range<f64> = Range {
        min: movement_constants::MAX_ROTATION,
        max: movement_constants::MAX_ROTATION,
    };
    pub const DIRECTIONAL: bool = true;
    pub const AGENT: Agent = Agent {
        max_speed: MAX_SPEED,
        max_force: MAX_FORCE,
        max_rotation: MAX_ROTATION,
        directional: DIRECTIONAL,
    };
    pub const VERTICES: Range<usize> = Range { min: 3, max: 8 };
    pub const NTH: Range<usize> = Range { min: 1, max: 3 };
    pub const NORMALISED_DIMENSIONS: Point2<f64> = Point2 { x: 1.0, y: 1.0 };
    pub const RADIANS_OFFSET: Range<f64> = Range {
        min: std::f64::consts::PI * 0.5,
        max: std::f64::consts::PI * 0.5,
    };
    pub const SPEED: Range<f64> = Range { min: 1.0, max: 5.0 };
    pub const NGON: Ngon = Ngon {
        vertices: VERTICES,
        nth: NTH,
        normalised_dimensions: NORMALISED_DIMENSIONS,
        radians_offset: RADIANS_OFFSET,
        speed: SPEED,
    };
    pub const GENERATIVE: Generative = Generative::Agent(AGENT);
    pub const MOVEMENT: Movement = Movement::Fixed(FIXED);

    pub fn spread() -> Metres { SPREAD }
    pub fn channel_radians() -> f32 { CHANNEL_RADIANS }
    pub fn volume() -> f32 { VOLUME }
    pub fn occurrence_rate() -> Range<Ms> { OCCURRENCE_RATE }
    pub fn simultaneous_sounds() -> Range<usize> { SIMULTANEOUS_SOUNDS }
    pub fn playback_duration() -> Range<Ms> { PLAYBACK_DURATION }
    pub fn attack_duration() -> Range<Ms> { ATTACK_DURATION }
    pub fn release_duration() -> Range<Ms> { RELEASE_DURATION }
    pub fn movement() -> Movement { MOVEMENT }
    pub fn radians_offset() -> Range<f64> { RADIANS_OFFSET }
    pub fn max_speed() -> Range<f64> { MAX_SPEED }
    pub fn max_force() -> Range<f64> { MAX_FORCE }
    pub fn max_rotation() -> Range<f64> { MAX_ROTATION }
    pub fn directional() -> bool { DIRECTIONAL }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundscape_role_round_trip() {
        let sc = Soundscape::default();
        let json = serde_json::to_string(&sc).unwrap();
        let back: Soundscape = serde_json::from_str(&json).unwrap();
        assert_eq!(sc.simultaneous_sounds, back.simultaneous_sounds);
    }

    #[test]
    fn movement_fixed_round_trip() {
        let m = Movement::Fixed(Point2 { x: 0.5, y: 0.3 });
        let json = serde_json::to_string(&m).unwrap();
        let back: Movement = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn movement_agent_round_trip() {
        let m = Movement::Generative(Generative::Agent(default::AGENT));
        let json = serde_json::to_string(&m).unwrap();
        let back: Movement = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
