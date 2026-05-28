use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs, io::{self, Write as IoWrite}, ops, path::Path, time};

// ── Time constants ────────────────────────────────────────────────────────────

pub const SEC_MS: f64 = 1_000.0;
pub const MIN_MS: f64 = SEC_MS * 60.0;
pub const HR_MS: f64 = MIN_MS * 60.0;
pub const DAY_MS: f64 = HR_MS * 24.0;

pub const MS_IN_HZ: f64 = 1_000.0;
pub const SEC_IN_HZ: f64 = 1.0;
pub const MIN_IN_HZ: f64 = SEC_IN_HZ / 60.0;
pub const HR_IN_HZ: f64 = MIN_IN_HZ / 60.0;
pub const DAY_IN_HZ: f64 = HR_IN_HZ / 24.0;

// ── Seed ─────────────────────────────────────────────────────────────────────

/// Seed for the project's deterministic RNG.  Preserved as `[u8; 16]` for JSON compatibility
/// with existing project files (previously used for XorShiftRng).
pub type Seed = [u8; 16];

// ── Ms ───────────────────────────────────────────────────────────────────────

/// Time in milliseconds.  Replaces `time_calc::Ms`; serialises as a plain f64.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Ms(pub f64);

impl Ms {
    pub fn ms(self) -> f64 {
        self.0
    }

    pub fn to_samples(self, sample_hz: f64) -> i64 {
        (self.0 / SEC_MS * sample_hz) as i64
    }
}

impl ops::Add for Ms {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Ms(self.0 + rhs.0) }
}

impl ops::Sub for Ms {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Ms(self.0 - rhs.0) }
}

impl ops::Mul<f64> for Ms {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self { Ms(self.0 * rhs) }
}

impl ops::Div<f64> for Ms {
    type Output = Self;
    fn div(self, rhs: f64) -> Self { Ms(self.0 / rhs) }
}

impl ops::AddAssign for Ms {
    fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
}

impl ops::SubAssign for Ms {
    fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
}

// ── Range<T> ─────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Range<T> {
    pub min: T,
    pub max: T,
}

impl<T: Clone + PartialOrd> Range<T> {
    pub fn clamp(&self, value: T) -> T {
        if value < self.min {
            self.min.clone()
        } else if value > self.max {
            self.max.clone()
        } else {
            value
        }
    }
}

// ── Human-readable time ───────────────────────────────────────────────────────

pub enum HumanReadableTime { Ms, Secs, Mins, Hrs, Days }

impl HumanReadableTime {
    pub fn times_per_unit_to_hz(&self, times_per_unit: f64) -> f64 {
        match self {
            HumanReadableTime::Ms   => times_per_unit * MS_IN_HZ,
            HumanReadableTime::Secs => times_per_unit,
            HumanReadableTime::Mins => times_per_unit * MIN_IN_HZ,
            HumanReadableTime::Hrs  => times_per_unit * HR_IN_HZ,
            HumanReadableTime::Days => times_per_unit * DAY_IN_HZ,
        }
    }

    pub fn to_ms(&self, value: f64) -> super::utils::Ms {
        match self {
            HumanReadableTime::Ms   => super::utils::Ms(value),
            HumanReadableTime::Secs => super::utils::Ms(value * SEC_MS),
            HumanReadableTime::Mins => super::utils::Ms(value * MIN_MS),
            HumanReadableTime::Hrs  => super::utils::Ms(value * HR_MS),
            HumanReadableTime::Days => super::utils::Ms(value * DAY_MS),
        }
    }
}

pub fn human_readable_hz(hz: f64) -> (HumanReadableTime, f64) {
    if hz < DAY_IN_HZ {
        (HumanReadableTime::Days, hz / DAY_IN_HZ)
    } else if hz < HR_IN_HZ {
        (HumanReadableTime::Hrs, hz / HR_IN_HZ)
    } else if hz < MIN_IN_HZ {
        (HumanReadableTime::Mins, hz / MIN_IN_HZ)
    } else if hz < SEC_IN_HZ {
        (HumanReadableTime::Secs, hz)
    } else {
        (HumanReadableTime::Ms, hz / MS_IN_HZ)
    }
}

pub fn human_readable_ms(ms: Ms) -> (HumanReadableTime, f64) {
    let v = ms.0;
    if v < SEC_MS {
        (HumanReadableTime::Ms, v)
    } else if v < MIN_MS {
        (HumanReadableTime::Secs, v / SEC_MS)
    } else if v < HR_MS {
        (HumanReadableTime::Mins, v / MIN_MS)
    } else if v < DAY_MS {
        (HumanReadableTime::Hrs, v / HR_MS)
    } else {
        (HumanReadableTime::Days, v / DAY_MS)
    }
}

pub fn ms_interval_to_hz(ms: Ms) -> f64 {
    let secs = ms.0 / SEC_MS;
    1.0 / secs
}

pub fn hz_to_ms_interval(hz: f64) -> Ms {
    Ms(SEC_MS / hz)
}

// ── Math helpers ──────────────────────────────────────────────────────────────

pub fn duration_to_secs(d: time::Duration) -> f64 {
    d.as_secs_f64()
}

pub fn rad_mag_to_x_y(rad: f64, mag: f64) -> (f64, f64) {
    (rad.cos() * mag, rad.sin() * mag)
}

/// Map a value from [in_min, in_max] to [out_min, out_max].
pub fn map_range(val: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    (val - in_min) / (in_max - in_min) * (out_max - out_min) + out_min
}

pub fn unnormalise(normalised: f64, min: f64, max: f64) -> f64 {
    map_range(normalised, 0.0, 1.0, min, max)
}

pub fn unskew_and_unnormalise(skewed_normalised: f64, min: f64, max: f64, skew: f32) -> f64 {
    let unskewed = skewed_normalised.powf(1.0 / skew as f64);
    unnormalise(unskewed, min, max)
}

/// Combine two 16-byte seeds element-wise with wrapping addition.
pub fn add_seeds(a: &Seed, b: &Seed) -> Seed {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = a[i].wrapping_add(b[i]);
    }
    out
}

/// Count the number of leading elements that compare equal to the first element.
pub fn count_equal<T, F>(slice: &[T], cmp: F) -> usize
where
    F: Fn(&T, &T) -> std::cmp::Ordering,
{
    match slice.first() {
        None => 0,
        Some(first) => slice
            .iter()
            .take_while(|x| cmp(first, x) == std::cmp::Ordering::Equal)
            .count(),
    }
}

/// 1D smooth noise in [-1.0, 1.0] (replaces `mindtree_utils::noise_walk`).
///
/// Uses value noise with smoothstep interpolation. Deterministic for the same phase.
/// The signal evolves continuously — advance the phase slowly for a gradual drift.
pub fn noise_walk(phase: f64) -> f64 {
    let i = phase.floor() as i64;
    let frac = phase - i as f64;
    let t = frac * frac * (3.0 - 2.0 * frac); // smoothstep
    hash_f64(i) + t * (hash_f64(i + 1) - hash_f64(i))
}

fn hash_f64(n: i64) -> f64 {
    let mut h = n as u64;
    h = h.wrapping_add(0x9e3779b97f4a7c15);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

// ── Assets directory ─────────────────────────────────────────────────────────

/// Find the `assets/` directory.
///
/// Searches in order:
/// 1. Next to the running executable (installed / release build).
/// 2. Two directories above the executable (inside `target/debug/` or `target/release/`).
/// 3. The current working directory (running from the repo root with `cargo run`).
///
/// Returns a path to use regardless of whether it currently exists.
pub fn assets_dir() -> std::path::PathBuf {
    let candidates: &[fn() -> Option<std::path::PathBuf>] = &[
        || {
            std::env::current_exe().ok()
                .and_then(|p| p.parent().map(|d| d.join("assets")))
        },
        || {
            std::env::current_exe().ok()
                .and_then(|p| p.parent().and_then(|d| d.parent()).and_then(|d| d.parent()).map(|d| d.join("assets")))
        },
        || Some(std::path::PathBuf::from("assets")),
    ];
    for f in candidates {
        if let Some(p) = f() {
            if p.is_dir() {
                return p;
            }
        }
    }
    std::path::PathBuf::from("assets")
}

// ── Platform-aware hidden-file check ─────────────────────────────────────────

pub fn is_file_hidden(path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.file_name()
            .map(|n| n.to_string_lossy().starts_with('.'))
            .unwrap_or(false)
    }
}

// ── Safe JSON file I/O ────────────────────────────────────────────────────────

/// Write `content` to `path` via a `.tmp` file so a crash mid-write doesn't corrupt the target.
pub fn safe_file_save(path: &Path, content: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");

    if tmp.exists() {
        fs::remove_file(&tmp)?;
    }
    if let Some(dir) = path.parent() {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
    }

    let mut file = fs::File::create(&tmp)?;
    file.write_all(content)?;

    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn save_to_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let s = serde_json::to_string_pretty(value)?;
    safe_file_save(path, s.as_bytes())?;
    Ok(())
}

pub fn load_from_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path)?;
    let v = serde_json::from_reader(file)?;
    Ok(v)
}

pub fn load_from_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    load_from_json(path).unwrap_or_default()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_round_trip() {
        let ms = Ms(1234.5);
        let json = serde_json::to_string(&ms).unwrap();
        assert_eq!(json, "1234.5");
        let back: Ms = serde_json::from_str(&json).unwrap();
        assert_eq!(ms, back);
    }

    #[test]
    fn range_round_trip() {
        let r = Range { min: Ms(100.0), max: Ms(5000.0) };
        let json = serde_json::to_string(&r).unwrap();
        let back: Range<Ms> = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn ms_arithmetic() {
        assert_eq!(Ms(1000.0) + Ms(500.0), Ms(1500.0));
        assert_eq!(Ms(1000.0) - Ms(200.0), Ms(800.0));
        assert_eq!(Ms(500.0) * 2.0, Ms(1000.0));
    }
}
