pub mod group;
pub mod movement;

use crate::audio::sound::{self, SoundCommand};
use crate::audio::source;
use crate::geom::Point2;
use crate::installation;
use crate::metres::Metres;
use crate::utils::{self, Ms, Range, Seed};
use crossbeam::channel::{self, Receiver, Sender};
use fxhash::FxHashMap;
use rand::{Rng, RngExt, SeedableRng};
use rand::rngs::SmallRng;
use std::ops;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use movement::Movement;
use movement::{agent::InstallationDataMap, BoundingRect};

const TICK_RATE_MS: u64 = 16;

// ── Type aliases ────────────────────────────────────────────────────────────────

type Installations = FxHashMap<installation::Id, installation::Soundscape>;
type Groups = FxHashMap<group::Id, group::Group>;
type Sources = FxHashMap<source::Id, Source>;
type Speakers = FxHashMap<crate::audio::speaker::Id, Speaker>;
type GroupsLastUsed = FxHashMap<group::Id, Instant>;
type SourcesLastUsed = FxHashMap<source::Id, Instant>;
type InstallationAreas = FxHashMap<installation::Id, movement::Area>;
type InstallationSpeakers = FxHashMap<installation::Id, Vec<crate::audio::speaker::Id>>;
type ActiveSounds = FxHashMap<sound::Id, ActiveSound>;
type ActiveSoundPositions = FxHashMap<sound::Id, ActiveSoundPosition>;
type ActiveSoundsPerInstallation = FxHashMap<installation::Id, Vec<sound::Id>>;
type TargetSoundsPerInstallation = FxHashMap<installation::Id, usize>;
type AvailableGroups = Vec<AvailableGroup>;
type AvailableSources = Vec<AvailableSource>;

// ── Message ─────────────────────────────────────────────────────────────────────

pub enum Message {
    Update(UpdateFn),
    Tick(Tick),
    Play,
    Pause,
    Exit,
}

/// Wraps a `FnOnce(&mut Model)` so it can be sent across a channel.
pub struct UpdateFn(Box<dyn FnMut(&mut Model) + Send>);

impl UpdateFn {
    fn call(&mut self, model: &mut Model) {
        (self.0)(model)
    }
}

impl<F: FnOnce(&mut Model) + Send + 'static> From<F> for UpdateFn {
    fn from(f: F) -> Self {
        let mut slot = Some(f);
        UpdateFn(Box::new(move |m| {
            if let Some(f) = slot.take() { f(m); }
        }))
    }
}

// ── Tick ────────────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug)]
pub struct Tick {
    pub instant: Instant,
    pub since_last_tick: Duration,
    pub playback_duration: Duration,
}

// ── Domain types within the soundscape thread ────────────────────────────────

/// Speaker state relevant to the soundscape (read-only copy from the project).
#[derive(Clone, Debug)]
pub struct Speaker {
    pub point: Point2<Metres>,
    pub installations: fxhash::FxHashSet<installation::Id>,
}

/// Source state relevant to the soundscape, including the Soundscape role constraints.
pub struct Source {
    pub constraints: source::Soundscape,
    pub kind: source::Kind,
    pub spread: Metres,
    pub channel_radians: f32,
    pub volume: f32,
    pub muted: bool,
    pub last_sound_created: Option<Instant>,
}

/// A sound currently playing, tracked by the soundscape thread.
pub struct ActiveSound {
    pub initial_installation: installation::Id,
    pub movement: Movement,
    pub handle: sound::Handle,
    pub started_at: Instant,
    pub duration_ms: Option<Ms>,
}

impl ActiveSound {
    pub fn position(&self) -> sound::Position {
        self.movement.position()
    }
}

struct ActiveSoundPosition {
    source_id: source::Id,
    position: sound::Position,
}

// ── Suitability helpers ───────────────────────────────────────────────────────

#[derive(Debug)]
struct Suitability {
    num_sounds_needed: usize,
    timing: Option<Timing>,
}

#[derive(Debug)]
struct Timing {
    duration_until_sound_needed: Ms,
}

#[derive(Debug)]
struct AvailableGroup {
    id: group::Id,
    suitability: Suitability,
}

#[derive(Debug)]
struct AvailableSource {
    id: source::Id,
    suitability: Suitability,
    playback_duration: Range<Ms>,
    attack_duration: Range<Ms>,
    release_duration: Range<Ms>,
}

fn suitability_order(a: &Suitability, b: &Suitability) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match b.num_sounds_needed.cmp(&a.num_sounds_needed) {
        Equal => match (&a.timing, &b.timing) {
            (None, Some(_)) => Less,
            (Some(_), None) => Greater,
            (None, None) => Equal,
            (Some(a), Some(b)) => a.duration_until_sound_needed
                .partial_cmp(&b.duration_until_sound_needed)
                .unwrap_or(Equal),
        },
        ord => ord,
    }
}

// ── Model ────────────────────────────────────────────────────────────────────

pub struct Model {
    seed: Seed,
    /// Persistent RNG seeded once at construction — never reseeded per-tick.
    rng: SmallRng,
    sound_id_gen: sound::IdGenerator,
    playback_duration: Duration,
    installations: Installations,
    groups: Groups,
    sources: Sources,
    speakers: Speakers,
    groups_last_used: GroupsLastUsed,
    sources_last_used: SourcesLastUsed,
    active_sounds: ActiveSounds,

    // intermediary buffers
    installation_speakers: InstallationSpeakers,
    installation_areas: InstallationAreas,
    target_sounds_per_installation: TargetSoundsPerInstallation,
    active_sounds_per_installation: ActiveSoundsPerInstallation,
    active_sound_positions: ActiveSoundPositions,
    available_groups: AvailableGroups,
    available_sources: AvailableSources,
    /// Scratch buffer — reused each tick to avoid per-tick Vec allocation.
    expired_scratch: Vec<sound::Id>,
    /// Scratch buffer — reused each tick to avoid cloning target_sounds_per_installation.
    inst_target_scratch: Vec<(installation::Id, usize)>,

    // channels to other threads
    wav_reader: crate::audio::source::wav::reader::Handle,
    sound_cmd_tx: Sender<SoundCommand>,
}

impl Model {
    pub fn insert_installation(&mut self, id: installation::Id, s: installation::Soundscape) {
        self.installations.insert(id, s);
    }
    pub fn remove_installation(&mut self, id: &installation::Id) {
        for spk in self.speakers.values_mut() { spk.installations.remove(id); }
        for src in self.sources.values_mut() { src.constraints.installations.remove(id); }
        self.installations.remove(id);
    }
    pub fn insert_group(&mut self, id: group::Id, g: group::Group) { self.groups.insert(id, g); }
    pub fn remove_group(&mut self, id: &group::Id) { self.groups.remove(id); }
    pub fn insert_speaker(&mut self, id: crate::audio::speaker::Id, s: Speaker) { self.speakers.insert(id, s); }
    pub fn remove_speaker(&mut self, id: &crate::audio::speaker::Id) { self.speakers.remove(id); }
    pub fn insert_source(&mut self, id: source::Id, s: Source) { self.sources.insert(id, s); }
    pub fn remove_source(&mut self, id: &source::Id) {
        self.active_sounds.retain(|_, s| s.handle.source_id() != *id);
        self.sources.remove(id);
    }
    pub fn clear_project_specific_data(&mut self) {
        self.installations.clear(); self.groups.clear(); self.sources.clear();
        self.speakers.clear(); self.groups_last_used.clear(); self.sources_last_used.clear();
        self.active_sounds.clear(); self.installation_speakers.clear();
        self.installation_areas.clear(); self.target_sounds_per_installation.clear();
        self.active_sounds_per_installation.clear(); self.active_sound_positions.clear();
        self.available_groups.clear(); self.available_sources.clear();
    }
}

// ── Soundscape handle ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Soundscape {
    tx: Sender<Message>,
    thread: Arc<Mutex<Option<JoinHandle<()>>>>,
    ticker_thread: Arc<Mutex<Option<JoinHandle<()>>>>,
    is_playing: Arc<AtomicBool>,
}

impl Soundscape {
    pub fn send<F>(&self, update: F) where F: FnOnce(&mut Model) + Send + 'static {
        let _ = self.tx.send(Message::Update(UpdateFn::from(update)));
    }
    pub fn is_playing(&self) -> bool { self.is_playing.load(Ordering::Relaxed) }
    pub fn play(&self) {
        self.is_playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Message::Play);
    }
    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::Relaxed);
        let _ = self.tx.send(Message::Pause);
    }
    pub fn exit(self) {
        let _ = self.tx.send(Message::Exit);
        // Join the main soundscape thread first — dropping its rx causes the ticker to exit too
        if let Some(h) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = h.join();
        }
        // Ticker exits within one TICK_RATE_MS after the main thread drops its receiver
        if let Some(h) = self.ticker_thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = h.join();
        }
    }
}

// ── spawn ────────────────────────────────────────────────────────────────────

pub fn spawn(
    seed: Seed,
    sound_id_gen: sound::IdGenerator,
    wav_reader: crate::audio::source::wav::reader::Handle,
    sound_cmd_tx: Sender<SoundCommand>,
) -> Soundscape {
    let is_playing = Arc::new(AtomicBool::new(true));

    // Internal message channel
    let (tx, rx) = channel::unbounded::<Message>();

    // Ticker thread — fires at 16ms intervals, sends Tick messages while playing
    let tick_tx = tx.clone();
    let tick_playing = Arc::clone(&is_playing);
    let ticker_handle = thread::Builder::new()
        .name("soundscape-ticker".into())
        .spawn(move || {
            let mut last = Instant::now();
            let mut playback_duration = Duration::ZERO;
            loop {
                thread::sleep(Duration::from_millis(TICK_RATE_MS));
                let now = Instant::now();
                let since_last = now.duration_since(last);
                last = now;
                if !tick_playing.load(Ordering::Relaxed) { continue; }
                playback_duration += since_last;
                let tick = Tick { instant: now, since_last_tick: since_last, playback_duration };
                if tick_tx.send(Message::Tick(tick)).is_err() { break; }
            }
        })
        .expect("failed to spawn soundscape-ticker");

    let rng = SmallRng::seed_from_u64(
        u64::from_le_bytes(seed[..8].try_into().unwrap())
    );

    let model = Model {
        seed,
        rng,
        sound_id_gen,
        playback_duration: Duration::ZERO,
        installations: Default::default(),
        groups: Default::default(),
        sources: Default::default(),
        speakers: Default::default(),
        groups_last_used: Default::default(),
        sources_last_used: Default::default(),
        active_sounds: Default::default(),
        installation_speakers: Default::default(),
        installation_areas: Default::default(),
        target_sounds_per_installation: Default::default(),
        active_sounds_per_installation: Default::default(),
        active_sound_positions: Default::default(),
        available_groups: Default::default(),
        available_sources: Default::default(),
        expired_scratch: Vec::new(),
        inst_target_scratch: Vec::new(),
        wav_reader,
        sound_cmd_tx,
    };

    let handle = thread::Builder::new()
        .name("soundscape".into())
        .spawn(move || run(model, rx))
        .expect("failed to spawn soundscape thread");

    Soundscape {
        tx,
        thread: Arc::new(Mutex::new(Some(handle))),
        ticker_thread: Arc::new(Mutex::new(Some(ticker_handle))),
        is_playing,
    }
}

// ── Thread loop ───────────────────────────────────────────────────────────────

fn run(mut model: Model, rx: Receiver<Message>) {
    for msg in rx {
        match msg {
            Message::Update(mut f) => f.call(&mut model),
            Message::Exit => break,
            Message::Tick(t) => tick(&mut model, t),
            Message::Play => {} // play/pause affects the ticker, not sounds yet
            Message::Pause => {}
        }
    }
}

// ── Tick ──────────────────────────────────────────────────────────────────────

fn tick(model: &mut Model, tick: Tick) {
    model.playback_duration = tick.playback_duration;

    update_installation_speakers(&model.speakers, &mut model.installation_speakers);
    update_installation_areas(&model.speakers, &model.installation_speakers, &mut model.installation_areas);
    update_target_sounds_per_installation(
        model.seed,
        &tick.playback_duration,
        &model.installations,
        &model.installation_areas,
        &mut model.target_sounds_per_installation,
    );

    // Expire finished sounds — reuse scratch Vec to avoid per-tick allocation.
    model.expired_scratch.clear();
    for (&id, s) in &model.active_sounds {
        if let Some(dur) = s.duration_ms {
            let elapsed_ms = Ms(s.started_at.elapsed().as_secs_f64() * 1000.0);
            if elapsed_ms >= dur {
                model.expired_scratch.push(id);
            }
        }
    }
    for i in 0..model.expired_scratch.len() {
        let id = model.expired_scratch[i];
        model.active_sounds.remove(&id);
        let _ = model.sound_cmd_tx.send(SoundCommand::Despawn(id));
    }

    update_active_sound_positions(&model.active_sounds, &mut model.active_sound_positions);
    for (sound_id, sound) in model.active_sounds.iter_mut() {
        match &mut sound.movement {
            Movement::Fixed(_) => {}
            Movement::Generative(movement::Generative::Agent(agent)) => {
                let inst_data = build_agent_data(
                    sound.handle.source_id(),
                    &model.sources,
                    &model.installations,
                    &model.installation_areas,
                    &model.target_sounds_per_installation,
                    &model.active_sound_positions,
                );
                agent.update(&mut model.rng, &tick.since_last_tick, &inst_data);
            }
            Movement::Generative(movement::Generative::Ngon(ngon)) => {
                if let Some(area) = model.installation_areas.get(&sound.initial_installation) {
                    ngon.update(&tick.since_last_tick, &area.bounding_rect);
                }
            }
        }
        let position = sound.position();
        let _ = model.sound_cmd_tx.send(SoundCommand::UpdatePosition { id: *sound_id, position });
    }

    update_active_sound_positions(&model.active_sounds, &mut model.active_sound_positions);
    update_active_sounds_per_installation(
        &model.active_sound_positions,
        &model.sources,
        &model.installation_areas,
        &mut model.active_sounds_per_installation,
    );

    // Spawn new sounds.
    // Collect (installation, target_count) into a scratch Vec to avoid cloning the HashMap
    // while also needing mutable access to other model fields inside the loop.
    model.inst_target_scratch.clear();
    for (&inst, &count) in &model.target_sounds_per_installation {
        model.inst_target_scratch.push((inst, count));
    }

    'installations: for idx in 0..model.inst_target_scratch.len() {
        let (installation, num_target) = model.inst_target_scratch[idx];
        let num_active = model.active_sounds_per_installation.get(&installation).map(|v| v.len()).unwrap_or(0);
        if num_target <= num_active { continue; }

        let inst_area = match model.installation_areas.get(&installation) {
            Some(a) => *a,
            None => continue,
        };

        for _ in 0..(num_target - num_active) {
            update_available_groups(
                &tick,
                &model.sources,
                &model.groups,
                &model.active_sounds,
                &model.groups_last_used,
                &mut model.available_groups,
            );
            if model.available_groups.is_empty() { continue 'installations; }

            update_available_sources(
                &installation,
                &tick,
                &model.sources,
                &model.active_sounds,
                &model.sources_last_used,
                &model.available_groups,
                &mut model.available_sources,
            );
            if model.available_sources.is_empty() { continue 'installations; }

            model.available_groups.sort_by(|a, b| suitability_order(&a.suitability, &b.suitability));
            model.available_sources.sort_by(|a, b| suitability_order(&a.suitability, &b.suitability));

            let num_eq_groups = utils::count_equal(&model.available_groups, |a, b| {
                suitability_order(&a.suitability, &b.suitability)
            });
            let num_eq_sources = utils::count_equal(&model.available_sources, |a, b| {
                suitability_order(&a.suitability, &b.suitability)
            });

            let group_idx = model.rng.random_range(0..num_eq_groups);
            let source_idx = model.rng.random_range(0..num_eq_sources);
            let picked_source = &model.available_sources[source_idx];
            let source_id = picked_source.id;

            // Generate durations
            let playback_ms = random_ms(&mut model.rng, picked_source.playback_duration);
            let attack_ms = random_ms(&mut model.rng, picked_source.attack_duration);
            let release_ms = random_ms(&mut model.rng, picked_source.release_duration);
            let attack_frames = attack_ms.to_samples(crate::audio::SAMPLE_RATE);
            let release_frames = release_ms.to_samples(crate::audio::SAMPLE_RATE);
            let duration_frames = playback_ms.to_samples(crate::audio::SAMPLE_RATE);

            // Generate initial position
            let x_mag: f64 = model.rng.random();
            let y_mag: f64 = model.rng.random();
            let pos = sound::Position {
                point: Point2::new(
                    inst_area.bounding_rect.left + inst_area.bounding_rect.width() * x_mag,
                    inst_area.bounding_rect.bottom + inst_area.bounding_rect.height() * y_mag,
                ),
                radians: model.rng.random::<f32>() * std::f32::consts::TAU,
            };

            // Generate movement
            let movement = generate_movement(
                source_id,
                &model.sources,
                installation,
                &model.installations,
                &model.installation_areas,
                &model.target_sounds_per_installation,
                &model.active_sounds,
                &mut model.rng,
            );

            let sound_id = model.sound_id_gen.generate_next();

            // Determine source kind and trigger WAV decode if needed.
            let kind = match model.sources.get(&source_id) {
                Some(src) => match &src.kind {
                    source::Kind::Wav(wav) => {
                        model.wav_reader.load(source_id.0, wav.path.clone());
                        sound::AudioSourceKind::Wav { id: source_id.0 }
                    }
                    source::Kind::Realtime(rt) => {
                        sound::AudioSourceKind::Realtime { channels: rt.channels.clone() }
                    }
                },
                None => continue 'installations,
            };

            // Notify audio output thread.
            let _ = model.sound_cmd_tx.send(SoundCommand::Spawn {
                id: sound_id,
                source_id,
                kind,
                position: pos,
                attack_frames,
                release_frames,
                duration_frames: Some(duration_frames),
            });

            let active = ActiveSound {
                initial_installation: installation,
                movement,
                handle: sound::Handle { sound_id, source_id },
                started_at: tick.instant,
                duration_ms: Some(playback_ms),
            };

            model.groups_last_used.insert(model.available_groups[group_idx].id, tick.instant);
            model.sources_last_used.insert(source_id, tick.instant);
            model.active_sounds.insert(sound_id, active);
        }
    }
}

// ── Tick helper functions ─────────────────────────────────────────────────────

fn update_installation_speakers(
    speakers: &Speakers,
    installation_speakers: &mut InstallationSpeakers,
) {
    for v in installation_speakers.values_mut() { v.clear(); }
    for (&id, spk) in speakers {
        for &inst in &spk.installations {
            installation_speakers.entry(inst).or_default().push(id);
        }
    }
}

fn update_installation_areas(
    speakers: &Speakers,
    installation_speakers: &InstallationSpeakers,
    installation_areas: &mut InstallationAreas,
) {
    installation_areas.clear();
    for (&inst, speaker_ids) in installation_speakers {
        let points = speaker_ids.iter().map(|id| speakers[id].point);
        let bounding_rect = match BoundingRect::from_points(points) {
            None => continue,
            Some(r) => r,
        };
        let centroid = centroid_of(speaker_ids.iter().map(|id| speakers[id].point));
        let area = movement::Area { bounding_rect, centroid };
        installation_areas.insert(inst, area);
    }
}

fn centroid_of(points: impl Iterator<Item = Point2<Metres>>) -> Point2<Metres> {
    let mut sum = [0.0f64; 2];
    let mut count = 0usize;
    for p in points {
        sum[0] += p.x.0;
        sum[1] += p.y.0;
        count += 1;
    }
    if count == 0 {
        return Point2::new(Metres(0.0), Metres(0.0));
    }
    Point2::new(Metres(sum[0] / count as f64), Metres(sum[1] / count as f64))
}

fn installation_seed(id: &installation::Id) -> Seed {
    let u = (id.0 % 256) as u8;
    [u; 16]
}

fn installation_target_sounds(
    seed: Seed,
    playback_duration: &Duration,
    installation: &installation::Id,
    constraints: &installation::Soundscape,
    installation_areas: &InstallationAreas,
) -> usize {
    if !installation_areas.contains_key(installation) { return 0; }
    let playback_secs = playback_duration.as_secs_f64();
    let hz = 1.0 / (60.0 * 60.0);
    let combined = utils::add_seeds(&seed, &installation_seed(installation));
    let rng_seed = u64::from_le_bytes(if combined == [0; 16] {
        let mut s = combined; s[0] = 1; s[..8].try_into().unwrap()
    } else {
        combined[..8].try_into().unwrap()
    });
    let phase_offset = rand::rngs::SmallRng::seed_from_u64(rng_seed).random::<f64>();
    let phase = phase_offset + playback_secs * hz;
    let amp = (utils::noise_walk(phase) * 1.5_f64).clamp(-1.0, 1.0);
    let normalised = amp * 0.5 + 0.5;
    let range = &constraints.simultaneous_sounds;
    (range.min as f64 + normalised * (range.max - range.min) as f64) as usize
}

fn update_target_sounds_per_installation(
    seed: Seed,
    playback_duration: &Duration,
    installations: &Installations,
    installation_areas: &InstallationAreas,
    target: &mut TargetSoundsPerInstallation,
) {
    target.clear();
    for (inst, constraints) in installations {
        let n = installation_target_sounds(seed, playback_duration, inst, constraints, installation_areas);
        target.insert(*inst, n);
    }
}

fn update_active_sound_positions(active: &ActiveSounds, positions: &mut ActiveSoundPositions) {
    positions.clear();
    for (&id, sound) in active {
        positions.insert(id, ActiveSoundPosition {
            source_id: sound.handle.source_id(),
            position: sound.position(),
        });
    }
}

fn update_active_sounds_per_installation(
    positions: &ActiveSoundPositions,
    sources: &Sources,
    areas: &InstallationAreas,
    per_inst: &mut ActiveSoundsPerInstallation,
) {
    for v in per_inst.values_mut() { v.clear(); }
    for (&id, pos) in positions {
        if let Some(inst) = closest_assigned_installation(pos, sources, areas) {
            per_inst.entry(inst).or_default().push(id);
        }
    }
}

fn closest_assigned_installation(
    pos: &ActiveSoundPosition,
    sources: &Sources,
    areas: &InstallationAreas,
) -> Option<installation::Id> {
    let src = sources.get(&pos.source_id)?;
    let sp = [pos.position.point.x.0, pos.position.point.y.0];
    src.constraints.installations.iter()
        .filter_map(|&i| areas.get(&i).map(|a| {
            let c = [a.centroid.x.0, a.centroid.y.0];
            let dx = sp[0] - c[0]; let dy = sp[1] - c[1];
            (i, dx*dx + dy*dy)
        }))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
}

fn build_agent_data(
    source_id: source::Id,
    sources: &Sources,
    installations: &Installations,
    areas: &InstallationAreas,
    target_per_inst: &TargetSoundsPerInstallation,
    positions: &ActiveSoundPositions,
) -> InstallationDataMap {
    let src = match sources.get(&source_id) { None => return Default::default(), Some(s) => s };
    let mut per_inst: ActiveSoundsPerInstallation = Default::default();
    for (&id, pos) in positions {
        if let Some(inst) = closest_assigned_installation(pos, sources, areas) {
            per_inst.entry(inst).or_default().push(id);
        }
    }
    src.constraints.installations.iter().filter_map(|inst| {
        let area = *areas.get(inst)?;
        let range = &installations.get(inst)?.simultaneous_sounds;
        let current = per_inst.get(inst).map(|v| v.iter().filter(|&&s| positions[&s].source_id == source_id).count()).unwrap_or(0);
        let target = *target_per_inst.get(inst).unwrap_or(&0);
        Some((*inst, movement::agent::InstallationData {
            area,
            num_sounds_needed_to_reach_target: target as i32 - current as i32,
            num_sounds_needed: if current < range.min { range.min - current } else { 0 },
            num_available_sounds: if current < range.max { range.max - current } else { 0 },
        }))
    }).collect()
}

fn update_available_groups(
    tick: &Tick,
    sources: &Sources,
    groups: &Groups,
    active_sounds: &ActiveSounds,
    groups_last_used: &GroupsLastUsed,
    available_groups: &mut AvailableGroups,
) {
    available_groups.clear();
    available_groups.extend(groups.iter().filter_map(|(group_id, group)| {
        let num_active = active_sounds.values().filter(|s| {
            sources.get(&s.handle.source_id()).map(|src| src.constraints.groups.contains(group_id)).unwrap_or(false)
        }).count();
        let num_available = group.simultaneous_sounds.max.checked_sub(num_active)?;
        if num_available == 0 { return None; }
        let num_sounds_needed = group.simultaneous_sounds.min.saturating_sub(num_active);
        let timing = groups_last_used.get(group_id).and_then(|&last| {
            let since_ms = Ms(tick.instant.duration_since(last).as_secs_f64() * 1000.0);
            if since_ms <= group.occurrence_rate.min { return None; }
            Some(Timing { duration_until_sound_needed: group.occurrence_rate.max - since_ms })
        });
        Some(AvailableGroup {
            id: *group_id,
            suitability: Suitability { num_sounds_needed, timing },
        })
    }));
}

fn update_available_sources(
    installation: &installation::Id,
    tick: &Tick,
    sources: &Sources,
    active_sounds: &ActiveSounds,
    sources_last_used: &SourcesLastUsed,
    available_groups: &AvailableGroups,
    available_sources: &mut AvailableSources,
) {
    available_sources.clear();
    available_sources.extend(sources.iter().filter_map(|(source_id, source)| {
        if !source.constraints.installations.contains(installation) { return None; }
        if available_groups.iter().all(|g| !source.constraints.groups.contains(&g.id)) { return None; }
        let num_active = active_sounds.values().filter(|s| s.handle.source_id() == *source_id).count();
        let num_available = source.constraints.simultaneous_sounds.max.checked_sub(num_active)?;
        if num_available == 0 { return None; }
        let num_sounds_needed = source.constraints.simultaneous_sounds.min.saturating_sub(num_active);
        let timing = sources_last_used.get(source_id).and_then(|&last| {
            let since_ms = Ms(tick.instant.duration_since(last).as_secs_f64() * 1000.0);
            if since_ms <= source.constraints.occurrence_rate.min { return None; }
            Some(Timing { duration_until_sound_needed: source.constraints.occurrence_rate.max - since_ms })
        });
        Some(AvailableSource {
            id: *source_id,
            suitability: Suitability { num_sounds_needed, timing },
            playback_duration: source.constraints.playback_duration,
            attack_duration: source.constraints.attack_duration,
            release_duration: source.constraints.release_duration,
        })
    }));
}

fn generate_movement<R: Rng>(
    source_id: source::Id,
    sources: &Sources,
    installation: installation::Id,
    installations: &Installations,
    areas: &InstallationAreas,
    target_per_inst: &TargetSoundsPerInstallation,
    active_sounds: &ActiveSounds,
    rng: &mut R,
) -> Movement {
    let src = match sources.get(&source_id) {
        Some(s) => s,
        None => return Movement::Fixed(sound::Position::default()),
    };
    match &src.constraints.movement {
        source::Movement::Fixed(norm_pos) => {
            let area = match areas.get(&installation) {
                Some(a) => a,
                None => return Movement::Fixed(sound::Position::default()),
            };
            let x = area.bounding_rect.left + area.bounding_rect.width() * norm_pos.x;
            let y = area.bounding_rect.bottom + area.bounding_rect.height() * norm_pos.y;
            Movement::Fixed(sound::Position { point: Point2::new(x, y), radians: 0.0 })
        }
        source::Movement::Generative(gen) => match gen {
            source::Generative::Agent(agent_params) => {
                let max_speed = lerp_range(rng.random::<f64>(), agent_params.max_speed);
                let max_force = lerp_range(rng.random::<f64>(), agent_params.max_force);
                let max_rotation = lerp_range(rng.random::<f64>(), agent_params.max_rotation);
                let mut positions: ActiveSoundPositions = Default::default();
                update_active_sound_positions(active_sounds, &mut positions);
                let inst_data = build_agent_data(source_id, sources, installations, areas, target_per_inst, &positions);
                let agent = movement::Agent::generate(
                    &mut *rng,
                    installation,
                    &inst_data,
                    max_speed,
                    max_force,
                    max_rotation,
                    agent_params.directional,
                );
                Movement::Generative(movement::Generative::Agent(agent))
            }
            source::Generative::Ngon(ngon_params) => {
                let vertices = lerp_range(rng.random::<f64>(), Range { min: ngon_params.vertices.min as f64, max: ngon_params.vertices.max as f64 }) as usize;
                let nth = lerp_range(rng.random::<f64>(), Range { min: ngon_params.nth.min as f64, max: ngon_params.nth.max as f64 }) as usize;
                let speed = lerp_range(rng.random::<f64>(), ngon_params.speed);
                let radians_offset = lerp_range(rng.random::<f64>(), ngon_params.radians_offset);
                let rect = match areas.get(&installation) {
                    Some(a) => a.bounding_rect,
                    None => return Movement::Fixed(sound::Position::default()),
                };
                let vertices = vertices.max(3);
                let nth = nth.max(1);
                let ngon = movement::Ngon::new(
                    vertices, nth,
                    [ngon_params.normalised_dimensions.x, ngon_params.normalised_dimensions.y],
                    radians_offset, speed, &rect,
                );
                Movement::Generative(movement::Generative::Ngon(ngon))
            }
        },
    }
}

fn lerp_range(t: f64, r: Range<f64>) -> f64 {
    r.min + t * (r.max - r.min)
}

fn random_ms<R: Rng>(rng: &mut R, range: Range<Ms>) -> Ms {
    Ms(range.min.0 + rng.random::<f64>() * (range.max.0 - range.min.0))
}

// ── Deref: soundscape::Source → audio::source::Soundscape ───────────────────

impl ops::Deref for Source {
    type Target = source::Soundscape;
    fn deref(&self) -> &Self::Target { &self.constraints }
}

impl ops::DerefMut for Source {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.constraints }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_walk_in_range() {
        for i in 0..100 {
            let v = utils::noise_walk(i as f64 * 0.1);
            assert!(v >= -1.0 && v <= 1.0, "noise_walk({}) = {v} out of [-1, 1]", i as f64 * 0.1);
        }
    }

    #[test]
    fn noise_walk_smooth() {
        let a = utils::noise_walk(0.0);
        let b = utils::noise_walk(0.001);
        assert!((a - b).abs() < 0.1, "noise_walk jumped too much between adjacent phases");
    }

    #[test]
    fn soundscape_thread_starts_and_exits() {
        let (cmd_tx, _cmd_rx) = crossbeam::channel::unbounded::<sound::SoundCommand>();
        let (wav, _wav_rx) = crate::audio::source::wav::reader::spawn();
        let sc = spawn([1u8; 16], sound::IdGenerator::new(), wav.handle, cmd_tx);
        std::thread::sleep(std::time::Duration::from_millis(50));
        sc.exit();
    }

    #[test]
    fn bounding_rect_from_points() {
        use crate::metres::Metres;
        use movement::BoundingRect;
        let pts = vec![
            crate::geom::Point2::new(Metres(1.0), Metres(2.0)),
            crate::geom::Point2::new(Metres(-1.0), Metres(5.0)),
            crate::geom::Point2::new(Metres(3.0), Metres(0.0)),
        ];
        let r = BoundingRect::from_points(pts).unwrap();
        assert_eq!(r.left, Metres(-1.0));
        assert_eq!(r.right, Metres(3.0));
        assert_eq!(r.bottom, Metres(0.0));
        assert_eq!(r.top, Metres(5.0));
    }
}
