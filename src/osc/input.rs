use crossbeam::channel::{self, Receiver, Sender};
use rosc::decoder;
use std::net::UdpSocket;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const BP_ADDR: &str = "/bp";
const SOURCE_VOLUME_PREFIX: &str = "/source_volume/";
const MASTER_VOLUME_ADDR: &str = "/master_volume";
const PLAY_SOUNDSCAPE_ADDR: &str = "/play_soundscape";
const PAUSE_SOUNDSCAPE_ADDR: &str = "/pause_soundscape";

/// A record of a received OSC message shown in the OSC-in log panel.
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub from: String,
    pub addr: String,
    pub args: String,
}

/// A control command extracted from an OSC message.
#[derive(Clone, Debug)]
pub enum ControlMsg {
    /// `/bp/master_volume <Float 0..1>`
    MasterVolume(f32),
    /// `/bp/source_volume/<name> <Float>`
    SourceVolume { name: String, volume: f32 },
    /// `/bp/play_soundscape`
    PlaySoundscape,
    /// `/bp/pause_soundscape`
    PauseSoundscape,
}

pub struct Spawned {
    pub log_rx: Receiver<LogEntry>,
    pub control_rx: Receiver<ControlMsg>,
    shutdown: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Spawned {
    /// Signal the input thread to stop and return its `JoinHandle`.
    pub fn exit(self) -> JoinHandle<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        self.thread
    }
}

/// Bind a UDP socket on `port` and start the OSC receiver thread.
pub fn spawn(port: u16) -> anyhow::Result<Spawned> {
    let (log_tx, log_rx) = channel::bounded::<LogEntry>(256);
    let (control_tx, control_rx) = channel::bounded::<ControlMsg>(256);
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown);

    let socket = UdpSocket::bind(format!("0.0.0.0:{port}"))?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;

    let thread = thread::Builder::new()
        .name("osc-in".into())
        .spawn(move || input_loop(socket, log_tx, control_tx, shutdown_flag))
        .expect("failed to spawn osc-in thread");

    Ok(Spawned { log_rx, control_rx, shutdown, thread })
}

fn input_loop(
    socket: UdpSocket,
    log_tx: Sender<LogEntry>,
    control_tx: Sender<ControlMsg>,
    shutdown: Arc<AtomicBool>,
) {
    let mut buf = vec![0u8; 65535];
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let (size, from) = match socket.recv_from(&mut buf) {
            Ok(ok) => ok,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                eprintln!("osc-in recv error: {e}");
                break;
            }
        };
        match decoder::decode_udp(&buf[..size]) {
            Ok((_, packet)) => {
                for msg in flatten_packet(packet) {
                    let _ = log_tx.try_send(LogEntry {
                        from: from.to_string(),
                        addr: msg.addr.clone(),
                        args: format_args_(&msg.args),
                    });
                    if let Some(ctrl) = parse_control(&msg) {
                        let _ = control_tx.try_send(ctrl);
                    }
                }
            }
            Err(e) => eprintln!("osc-in decode error: {e}"),
        }
    }
}

fn flatten_packet(packet: rosc::OscPacket) -> Vec<rosc::OscMessage> {
    match packet {
        rosc::OscPacket::Message(msg) => vec![msg],
        rosc::OscPacket::Bundle(bundle) => {
            bundle.content.into_iter().flat_map(flatten_packet).collect()
        }
    }
}

fn parse_control(msg: &rosc::OscMessage) -> Option<ControlMsg> {
    let rest = msg.addr.strip_prefix(BP_ADDR)?;
    if rest == MASTER_VOLUME_ADDR {
        if let Some(rosc::OscType::Float(vol)) = msg.args.first() {
            return Some(ControlMsg::MasterVolume(vol.clamp(0.0, 1.0)));
        }
    }
    if let Some(name) = rest.strip_prefix(SOURCE_VOLUME_PREFIX) {
        if !name.is_empty() {
            if let Some(rosc::OscType::Float(volume)) = msg.args.first() {
                return Some(ControlMsg::SourceVolume {
                    name: name.to_string(),
                    volume: *volume,
                });
            }
        }
    }
    if rest == PLAY_SOUNDSCAPE_ADDR {
        return Some(ControlMsg::PlaySoundscape);
    }
    if rest == PAUSE_SOUNDSCAPE_ADDR {
        return Some(ControlMsg::PauseSoundscape);
    }
    None
}

fn format_args_(args: &[rosc::OscType]) -> String {
    args.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(addr: &str, args: Vec<rosc::OscType>) -> rosc::OscMessage {
        rosc::OscMessage { addr: addr.to_string(), args }
    }

    #[test]
    fn parses_master_volume() {
        let m = msg("/bp/master_volume", vec![rosc::OscType::Float(0.75)]);
        match parse_control(&m) {
            Some(ControlMsg::MasterVolume(v)) => assert!((v - 0.75).abs() < 1e-6),
            other => panic!("expected MasterVolume, got {other:?}"),
        }
    }

    #[test]
    fn parses_source_volume() {
        let m = msg("/bp/source_volume/ambient", vec![rosc::OscType::Float(0.5)]);
        match parse_control(&m) {
            Some(ControlMsg::SourceVolume { name, volume }) => {
                assert_eq!(name, "ambient");
                assert!((volume - 0.5).abs() < 1e-6);
            }
            other => panic!("expected SourceVolume, got {other:?}"),
        }
    }

    #[test]
    fn parses_play_pause() {
        assert!(matches!(
            parse_control(&msg("/bp/play_soundscape", vec![])),
            Some(ControlMsg::PlaySoundscape)
        ));
        assert!(matches!(
            parse_control(&msg("/bp/pause_soundscape", vec![])),
            Some(ControlMsg::PauseSoundscape)
        ));
    }

    #[test]
    fn ignores_unknown_addr() {
        assert!(parse_control(&msg("/something/else", vec![])).is_none());
        assert!(parse_control(&msg("/bp/unknown", vec![])).is_none());
    }

    #[test]
    fn master_volume_clamped() {
        let m = msg("/bp/master_volume", vec![rosc::OscType::Float(2.0)]);
        match parse_control(&m) {
            Some(ControlMsg::MasterVolume(v)) => assert_eq!(v, 1.0),
            _ => panic!(),
        }
    }
}
