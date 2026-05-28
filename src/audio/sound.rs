use crate::geom::Point2;
use crate::installation;
use crate::metres::Metres;
use fxhash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Sound Ids use a private u64 to match the original format.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Id(u64);

impl Id {
    const INITIAL: Self = Id(0);
}

/// Thread-safe generator; shared between the GUI and soundscape threads.
#[derive(Clone)]
pub struct IdGenerator {
    next: Arc<Mutex<Id>>,
}

impl IdGenerator {
    pub fn new() -> Self {
        IdGenerator { next: Arc::new(Mutex::new(Id::INITIAL)) }
    }
}

impl Default for IdGenerator {
    fn default() -> Self { Self::new() }
}

impl IdGenerator {

    pub fn generate_next(&self) -> Id {
        let mut n = self.next.lock().expect("sound::IdGenerator mutex poisoned");
        let id = *n;
        *n = Id(id.0.wrapping_add(1));
        id
    }
}

/// The spatial position and orientation of a sound within the exhibition.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub point: Point2<Metres>,
    #[serde(default)]
    pub radians: f32,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            point: Point2::new(Metres(0.0), Metres(0.0)),
            radians: 0.0,
        }
    }
}

/// Which installations a playing sound may be heard within.
pub enum Installations {
    All,
    Set(FxHashSet<installation::Id>),
}

/// A playing sound instance (runtime only — not persisted).
pub struct Sound;

/// A lightweight handle to a currently-active sound tracked by the soundscape thread.
pub struct Handle {
    pub sound_id: Id,
    pub source_id: crate::audio::source::Id,
}

impl Handle {
    pub fn sound_id(&self) -> Id { self.sound_id }
    pub fn source_id(&self) -> crate::audio::source::Id { self.source_id }
}

/// Commands sent from the soundscape thread to the audio output thread.
pub enum SoundCommand {
    /// Add a new sound to the audio mix.
    Spawn {
        id: Id,
        source_id: crate::audio::source::Id,
        position: Position,
        attack_frames: i64,
        release_frames: i64,
        /// `None` means play until explicitly despawned.
        duration_frames: Option<i64>,
    },
    /// Remove a playing sound.
    Despawn(Id),
    /// Move a playing sound.
    UpdatePosition { id: Id, position: Position },
}
