use crate::audio::detection::AudioFrameData;
use crate::installation;
use crossbeam::channel::{self, Receiver, Sender};
use fxhash::FxHashMap;
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::net::{SocketAddr, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Commands sent to the OSC output thread.
pub enum Message {
    /// New audio analysis frame for an installation — buffered and sent on next tick.
    Audio(installation::Id, AudioFrameData),
    /// Register an OSC target for an installation computer.
    AddTarget {
        installation: installation::Id,
        computer: installation::computer::Id,
        addr: SocketAddr,
        osc_addr: String,
    },
    /// Remove one computer from an installation's target list.
    RemoveTarget {
        installation: installation::Id,
        computer: installation::computer::Id,
    },
    /// Remove all computers for an installation.
    RemoveInstallation(installation::Id),
    /// Update the OSC address sent to a specific installation computer.
    UpdateAddr {
        installation: installation::Id,
        computer: installation::computer::Id,
        osc_addr: String,
    },
    /// Drop all per-project state (targets + pending frames).
    ClearProjectData,
    /// Shut down the thread.
    Exit,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub target: SocketAddr,
    pub osc_addr: String,
    pub error: bool,
}

pub struct Spawned {
    pub msg_tx: Sender<Message>,
    pub log_rx: Receiver<LogEntry>,
    thread: JoinHandle<()>,
}

impl Spawned {
    pub fn join(self) -> thread::Result<()> {
        let _ = self.msg_tx.send(Message::Exit);
        self.thread.join()
    }
}

/// Spawn the OSC output thread, returning channels for sending messages and reading logs.
pub fn spawn() -> Spawned {
    // Bounded so a stalled network cannot grow this channel without limit.
    // Producers must use try_send and accept that stale audio frames are dropped.
    let (msg_tx, msg_rx) = channel::bounded::<Message>(64);
    let (log_tx, log_rx) = channel::bounded::<LogEntry>(256);
    let thread = thread::Builder::new()
        .name("osc-out".into())
        .spawn(move || output_loop(msg_rx, log_tx))
        .expect("failed to spawn osc-out thread");
    Spawned { msg_tx, log_rx, thread }
}

struct Target {
    addr: SocketAddr,
    osc_addr: String,
}

fn output_loop(msg_rx: Receiver<Message>, log_tx: Sender<LogEntry>) {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("osc-out: failed to bind UDP socket: {e}");
            return;
        }
    };

    type TargetMap = FxHashMap<installation::computer::Id, Target>;
    let mut targets: FxHashMap<installation::Id, TargetMap> = FxHashMap::default();
    let mut pending: FxHashMap<installation::Id, AudioFrameData> = FxHashMap::default();

    let tick = Duration::from_millis(16);

    loop {
        crossbeam::select! {
            recv(msg_rx) -> result => {
                match result {
                    Err(_) | Ok(Message::Exit) => break,
                    Ok(Message::ClearProjectData) => {
                        targets.clear();
                        pending.clear();
                    }
                    Ok(Message::Audio(inst, data)) => {
                        pending.insert(inst, data);
                    }
                    Ok(Message::AddTarget { installation, computer, addr, osc_addr }) => {
                        targets.entry(installation).or_default().insert(computer, Target { addr, osc_addr });
                    }
                    Ok(Message::RemoveTarget { installation, computer }) => {
                        if let Some(m) = targets.get_mut(&installation) { m.remove(&computer); }
                    }
                    Ok(Message::RemoveInstallation(inst)) => { targets.remove(&inst); }
                    Ok(Message::UpdateAddr { installation, computer, osc_addr }) => {
                        if let Some(m) = targets.get_mut(&installation) {
                            if let Some(t) = m.get_mut(&computer) { t.osc_addr = osc_addr; }
                        }
                    }
                }
            }
            default(tick) => {
                flush(&socket, &mut pending, &targets, &log_tx);
            }
        }
    }
}

fn flush(
    socket: &UdpSocket,
    pending: &mut FxHashMap<installation::Id, AudioFrameData>,
    targets: &FxHashMap<installation::Id, FxHashMap<installation::computer::Id, Target>>,
    log_tx: &Sender<LogEntry>,
) {
    for (inst, data) in pending.drain() {
        let inst_targets = match targets.get(&inst) {
            Some(t) => t,
            None => continue,
        };
        let args = build_args(&data);
        for target in inst_targets.values() {
            let packet = OscPacket::Message(OscMessage {
                addr: target.osc_addr.clone(),
                args: args.clone(),
            });
            let error = match encoder::encode(&packet) {
                Ok(buf) => socket.send_to(&buf, target.addr).is_err(),
                Err(_) => true,
            };
            let _ = log_tx.try_send(LogEntry {
                target: target.addr,
                osc_addr: target.osc_addr.clone(),
                error,
            });
        }
    }
}

/// Build the OSC argument list from an `AudioFrameData`.
///
/// Wire format (matches original museum deployment):
///   avg_peak: Float, avg_rms: Float,
///   lmh[0..3]: Float×3, bins[0..8]: Float×8,
///   [speaker_index: Int, peak: Float, rms: Float] × N
fn build_args(data: &AudioFrameData) -> Vec<OscType> {
    let mut args = Vec::with_capacity(2 + 3 + 8 + data.speakers.len() * 3);
    args.push(OscType::Float(data.avg_peak));
    args.push(OscType::Float(data.avg_rms));
    for &v in &data.avg_fft.lmh { args.push(OscType::Float(v)); }
    for &v in &data.avg_fft.bins { args.push(OscType::Float(v)); }
    for (i, s) in data.speakers.iter().enumerate() {
        args.push(OscType::Int(i as i32));
        args.push(OscType::Float(s.peak));
        args.push(OscType::Float(s.rms));
    }
    args
}
