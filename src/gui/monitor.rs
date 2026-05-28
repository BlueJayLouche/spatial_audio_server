use crate::audio::detection::AudioFrameData;
use crate::audio::sound;
use crossbeam::channel::{self, Receiver, Sender};

/// Audio level data for a single active sound instance, shown on the floorplan canvas.
#[derive(Clone, Debug)]
pub struct ActiveSoundMonitor {
    pub source_id: crate::audio::source::Id,
    pub position: sound::Position,
    pub peak: f32,
    pub rms: f32,
}

/// A message sent from the audio output thread to the GUI.
#[derive(Clone, Debug)]
pub enum AudioMonitorMsg {
    /// Per-frame aggregate audio data for all installations.
    Frame(AudioFrameData),
    /// An active sound started or moved.
    SoundUpdate {
        id: sound::Id,
        monitor: ActiveSoundMonitor,
    },
    /// An active sound ended.
    SoundEnded(sound::Id),
}

pub type MsgSender = Sender<AudioMonitorMsg>;
pub type MsgReceiver = Receiver<AudioMonitorMsg>;

/// Create a linked sender/receiver pair for audio monitor messages.
pub fn channel() -> (MsgSender, MsgReceiver) {
    channel::bounded(512)
}
