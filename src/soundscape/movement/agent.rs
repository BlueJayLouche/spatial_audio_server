use crate::audio::sound::Position;
use crate::installation;
use crate::metres::Metres;
use fxhash::FxHashMap;
use rand::{Rng, RngExt};
use std::time::Duration;

use super::Area;

const TARGET_DISTANCE_THRESHOLD: f64 = 1.0; // metres

/// Per-installation data used by the Agent to choose targets and apply forces.
#[derive(Debug)]
pub struct InstallationData {
    pub area: Area,
    /// positive = installation needs more sounds, negative = too many
    pub num_sounds_needed_to_reach_target: i32,
    pub num_sounds_needed: usize,
    pub num_available_sounds: usize,
}

pub type InstallationDataMap = FxHashMap<installation::Id, InstallationData>;

/// Steering-behaviour agent that seeks a target within the assigned installations.
#[derive(Debug)]
pub struct Agent {
    location: [f64; 2],
    target_location: [f64; 2],
    velocity: [f64; 2],
    pub max_speed: f64,
    pub max_force: f64,
    pub max_rotation: f64,
    pub directional: bool,
}

impl Agent {
    pub fn generate<R: Rng>(
        mut rng: R,
        start_installation: installation::Id,
        installations: &InstallationDataMap,
        max_speed: f64,
        max_force: f64,
        max_rotation: f64,
        directional: bool,
    ) -> Self {
        let inst_data = installations
            .get(&start_installation)
            .expect("no InstallationData for start_installation");
        let location = random_in_area(&mut rng, &inst_data.area);
        let target_location = pick_target(&mut rng, installations);
        let start_magnitude = rng.random::<f64>() * max_speed;
        let desired = desired_velocity(location, target_location);
        let desired_angle = desired[1].atan2(desired[0]);
        let initial_angle = desired_angle + rng.random::<f64>() * 2.0 - 1.0;
        let velocity = [
            initial_angle.cos() * start_magnitude,
            initial_angle.sin() * start_magnitude,
        ];
        Agent { location, target_location, velocity, max_speed, max_force, max_rotation, directional }
    }

    pub fn position(&self) -> Position {
        let radians = if self.directional {
            self.velocity[1].atan2(self.velocity[0]) as f32
        } else {
            0.0
        };
        Position {
            point: crate::geom::Point2::new(Metres(self.location[0]), Metres(self.location[1])),
            radians,
        }
    }

    pub fn update<R: Rng>(
        &mut self,
        mut rng: R,
        delta_time: &Duration,
        installations: &InstallationDataMap,
    ) {
        if !installations.is_empty() {
            if should_pick_new_target(self.location, self.target_location, installations) {
                self.target_location = pick_target(&mut rng, installations);
            }
        }

        let force = seek_force(
            self.location,
            self.target_location,
            self.velocity,
            self.max_speed,
            self.max_force,
        );
        self.apply_force(force, delta_time);

        if reached_target(self.location, self.target_location) && !installations.is_empty() {
            self.target_location = pick_target(rng, installations);
        }
    }

    fn apply_force(&mut self, force: [f64; 2], delta_time: &Duration) {
        let dt = delta_time.as_secs_f64();
        let new_vel = add(self.velocity, force);

        // Limit angular change per second
        let angle_old = self.velocity[1].atan2(self.velocity[0]);
        let angle_new = new_vel[1].atan2(new_vel[0]);
        let mut delta = angle_new - angle_old;
        // Wrap to [-PI, PI]
        while delta > std::f64::consts::PI { delta -= 2.0 * std::f64::consts::PI; }
        while delta < -std::f64::consts::PI { delta += 2.0 * std::f64::consts::PI; }

        let max_delta = self.max_rotation * dt;
        let clamped_delta = delta.clamp(-max_delta, max_delta);
        let final_angle = angle_old + clamped_delta;
        let speed = magnitude(new_vel).min(self.max_speed);

        self.velocity = [final_angle.cos() * speed, final_angle.sin() * speed];
        self.location = add(self.location, scale(self.velocity, dt));
    }
}

fn desired_velocity(from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
    sub(to, from)
}

fn seek_force(
    pos: [f64; 2],
    target: [f64; 2],
    vel: [f64; 2],
    max_speed: f64,
    max_force: f64,
) -> [f64; 2] {
    let dv = normalize(desired_velocity(pos, target));
    let desired = scale(dv, max_speed);
    let steering = sub(desired, vel);
    limit_magnitude(steering, max_force)
}

fn should_pick_new_target(
    current: [f64; 2],
    target: [f64; 2],
    installations: &InstallationDataMap,
) -> bool {
    if let Some(target_inst) = closest_installation(target, installations) {
        if installations[target_inst].num_available_sounds == 0 {
            if let Some(current_inst) = closest_installation(current, installations) {
                return target_inst != current_inst;
            }
        }
    }
    false
}

fn closest_installation<'a>(
    p: [f64; 2],
    installations: &'a InstallationDataMap,
) -> Option<&'a installation::Id> {
    installations
        .iter()
        .min_by(|(_, a), (_, b)| {
            let da = distance2(p, centroid_arr(&a.area));
            let db = distance2(p, centroid_arr(&b.area));
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| id)
}

pub fn pick_target<R: Rng>(mut rng: R, installations: &InstallationDataMap) -> [f64; 2] {
    let mut vec: Vec<&InstallationData> = installations.values().collect();
    vec.sort_by(|a, b| installation_suitability(a, b));
    let index = ((rng.random::<f32>().powi(4)) * vec.len() as f32) as usize;
    let index = index.min(vec.len().saturating_sub(1));
    random_in_area(&mut rng, &vec[index].area)
}

fn installation_suitability(a: &InstallationData, b: &InstallationData) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a.num_available_sounds, b.num_available_sounds) {
        (_, 0) => return Less,
        (0, _) => return Greater,
        _ => {}
    }
    match b.num_sounds_needed.cmp(&a.num_sounds_needed) {
        Equal => {}
        ord => return ord,
    }
    b.num_sounds_needed_to_reach_target
        .cmp(&a.num_sounds_needed_to_reach_target)
}

fn random_in_area<R: Rng>(rng: &mut R, area: &Area) -> [f64; 2] {
    let r = &area.bounding_rect;
    let x = r.left.0 + (r.right.0 - r.left.0) * rng.random::<f64>();
    let y = r.bottom.0 + (r.top.0 - r.bottom.0) * rng.random::<f64>();
    [x, y]
}

fn reached_target(current: [f64; 2], target: [f64; 2]) -> bool {
    distance(current, target) <= TARGET_DISTANCE_THRESHOLD
}

fn centroid_arr(area: &Area) -> [f64; 2] {
    [area.centroid.x.0, area.centroid.y.0]
}

// ── 2D vector helpers ──────────────────────────────────────────────────────────

fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] { [a[0] + b[0], a[1] + b[1]] }
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] { [a[0] - b[0], a[1] - b[1]] }
fn scale(v: [f64; 2], s: f64) -> [f64; 2] { [v[0] * s, v[1] * s] }
fn magnitude(v: [f64; 2]) -> f64 { (v[0] * v[0] + v[1] * v[1]).sqrt() }
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 { magnitude(sub(a, b)) }
fn distance2(a: [f64; 2], b: [f64; 2]) -> f64 { let d = sub(a, b); d[0]*d[0] + d[1]*d[1] }
fn normalize(v: [f64; 2]) -> [f64; 2] {
    let m = magnitude(v);
    if m > 1e-10 { scale(v, 1.0 / m) } else { [0.0, 0.0] }
}
fn limit_magnitude(v: [f64; 2], limit: f64) -> [f64; 2] {
    let m = magnitude(v);
    if m > limit { scale(normalize(v), limit) } else { v }
}
