use crate::geom::Point2;
use crate::installation;
use crate::metres::Metres;
use fxhash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// Sound Ids use a private u64 to match the original format.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Id(u64);

/// Thread-safe generator; shared between the GUI and soundscape threads.
///
/// Uses an atomic counter — lock-free and safe to call from any thread,
/// including the audio thread, without risk of priority inversion.
#[derive(Clone)]
pub struct IdGenerator {
    next: Arc<AtomicU64>,
}

impl IdGenerator {
    pub fn new() -> Self {
        IdGenerator { next: Arc::new(AtomicU64::new(0)) }
    }

    pub fn generate_next(&self) -> Id {
        Id(self.next.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for IdGenerator {
    fn default() -> Self { Self::new() }
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
