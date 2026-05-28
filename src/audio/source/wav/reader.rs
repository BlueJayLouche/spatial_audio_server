use crossbeam::channel::{self, Receiver, Sender};
use hound::WavReader;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

/// Commands sent from the soundscape / GUI threads to the WAV reader.
pub enum Command {
    /// Load the WAV at `path`, identified by `id` (source::Id as u64).
    Load { id: u64, path: PathBuf },
    /// Unload the WAV with `id` from the in-memory cache.
    Unload { id: u64 },
    /// Shut down the reader thread.
    Exit,
}

/// Decoded f32 samples for one WAV file, sent to the output thread.
pub struct DecodedWav {
    pub id: u64,
    pub samples: Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
}

/// A cloneable handle to the WAV reader thread.
#[derive(Clone)]
pub struct Handle {
    command_tx: Sender<Command>,
}

impl Handle {
    pub fn load(&self, id: u64, path: PathBuf) {
        let _ = self.command_tx.send(Command::Load { id, path });
    }

    pub fn unload(&self, id: u64) {
        let _ = self.command_tx.send(Command::Unload { id });
    }


}

/// Returned by `spawn()` — holds the command handle and thread join handle.
///
/// `wav_rx` is returned separately so the audio output thread can own it.
pub struct Spawned {
    pub handle: Handle,
    thread: JoinHandle<()>,
}

impl Spawned {
    pub fn join(self) -> thread::Result<()> {
        self.thread.join()
    }

    /// Signal the reader thread to exit and join it.
    pub fn exit(self) -> thread::Result<()> {
        let _ = self.handle.command_tx.send(Command::Exit);
        self.thread.join()
    }
}

/// Spawn the WAV reader thread.
///
/// Returns `(Spawned, wav_rx)`. Pass `wav_rx` to the audio output thread so it
/// can cache decoded samples for mixing. `Spawned` keeps the command handle for
/// load/unload requests.
pub fn spawn() -> (Spawned, Receiver<DecodedWav>) {
    let (command_tx, command_rx) = channel::unbounded::<Command>();
    let (wav_tx, wav_rx) = channel::bounded::<DecodedWav>(64);

    let thread = thread::Builder::new()
        .name("wav-reader".into())
        .spawn(move || reader_loop(command_rx, wav_tx))
        .expect("failed to spawn wav-reader thread");

    (Spawned { handle: Handle { command_tx }, thread }, wav_rx)
}

fn reader_loop(command_rx: Receiver<Command>, wav_tx: Sender<DecodedWav>) {
    loop {
        match command_rx.recv() {
            Err(_) => break,
            Ok(Command::Exit) => break,
            Ok(Command::Unload { .. }) => {
                // The output thread is responsible for evicting cached samples;
                // the reader thread only decodes on demand.
            }
            Ok(Command::Load { id, path }) => {
                match decode_wav(id, &path) {
                    Ok(decoded) => {
                        let _ = wav_tx.send(decoded);
                    }
                    Err(e) => {
                        eprintln!("wav-reader: failed to load {:?}: {e}", path);
                    }
                }
            }
        }
    }
}

fn decode_wav(id: u64, path: &std::path::Path) -> anyhow::Result<DecodedWav> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
        }
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let scale = 1.0 / (1i64 << (bits - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap_or(0) as f32 * scale).collect()
        }
    };
    Ok(DecodedWav { id, samples, channels, sample_rate })
}
