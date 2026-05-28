//! Distance-Based Amplitude Panning (DBAP).
//!
//! Based on the algorithm published by Trond Lossius, 2009.
//! Positions are passed as `[f64; 2]` (x, y in metres) — no nannou dependency.

/// Per-speaker data required by the DBAP gain calculation.
#[derive(Copy, Clone, Debug)]
pub struct Speaker {
    /// Blurred squared distance from the virtual source to this speaker.
    /// Equal to `dx² + dy² + blur²`; always positive (guaranteed by `blurred_distance_2`).
    pub distance_sq: f64,
    /// Weighting factor (1.0 = full, 0.0 = silent — used for installation membership).
    pub weight: f64,
}

/// Iterator yielding the DBAP gain for each speaker in order.
#[derive(Clone)]
pub struct SpeakerGains<'a> {
    speakers: &'a [Speaker],
    a: f64,
    k: f64,
    i: usize,
}

impl<'a> SpeakerGains<'a> {
    pub fn new(speakers: &'a [Speaker], rolloff_db: f64) -> Self {
        debug_assert!(!speakers.is_empty());
        let a = a_coefficient(rolloff_db);
        let k = k_coefficient(a, speakers);
        SpeakerGains { speakers, a, k, i: 0 }
    }
}

impl<'a> Iterator for SpeakerGains<'a> {
    type Item = f64;
    fn next(&mut self) -> Option<Self::Item> {
        let s = self.speakers.get(self.i)?;
        self.i += 1;
        Some(v_speaker(s, self.k, self.a))
    }
}

/// Squared blurred distance between source and speaker.
///
/// The `blur` term prevents division by zero and models the "vertical displacement" from the
/// paper — larger blur = less extreme panning.
pub fn blurred_distance_2(src: [f64; 2], spk: [f64; 2], blur: f64) -> f64 {
    let dx = spk[0] - src[0];
    let dy = spk[1] - src[1];
    (dx * dx + dy * dy + blur * blur).max(f64::EPSILON)
}

/// Gain for speaker `s` — Lossius 2009, eq. 5.
///
/// `distance_sq = d_linear²`, so `d_linear^a = distance_sq^(a/2)`.
fn v_speaker(s: &Speaker, k: f64, a: f64) -> f64 {
    debug_assert!(s.distance_sq > 0.0);
    k * s.weight / s.distance_sq.powf(a / 2.0)
}

/// Exponent `a` for the DBAP distance law: `V ∝ d^(-a)`.
///
/// Derived from the rolloff in dB per doubling of distance:
///   `dB/doubling = 20·a·log₁₀(2)  →  a = rolloff_db / (20·log₁₀(2))`
///
/// Notable values: 0 dB → a=0 (flat), 6 dB → a≈1 (free-field inverse-distance law).
fn a_coefficient(rolloff_db: f64) -> f64 {
    rolloff_db / (20.0 * 2f64.log10())
}

/// Normalisation coefficient — Lossius 2009, eq. 6.
///
/// `k = 1 / sqrt( Σ w_j² / d_sq_j^a )` where `d_sq = d_linear²`.
/// Returns 0.0 if all speaker weights are zero.
fn k_coefficient(a: f64, speakers: &[Speaker]) -> f64 {
    debug_assert!(!speakers.is_empty());
    let sum = speakers.iter().fold(0.0_f64, |acc, s| {
        debug_assert!(s.distance_sq > 0.0);
        acc + s.weight.powi(2) / s.distance_sq.powf(a)
    });
    if sum == 0.0 { 0.0 } else { sum.sqrt().recip() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::DISTANCE_BLUR;

    fn dist(src: [f64; 2], spk: [f64; 2]) -> f64 {
        blurred_distance_2(src, spk, DISTANCE_BLUR)
    }

    #[test]
    fn equidistant_speakers_equal_gains() {
        let src = [5.0, 5.0_f64];
        let corners: &[[f64; 2]] = &[
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
        ];
        let speakers: Vec<Speaker> = corners
            .iter()
            .map(|&c| Speaker { distance_sq: dist(src, c), weight: 1.0 })
            .collect();

        let gains: Vec<f64> = SpeakerGains::new(&speakers, 6.0).collect();
        let g = gains[0];
        for gain in &gains {
            assert!((gain - g).abs() < 1e-10, "gains differ: {gains:?}");
        }
    }

    #[test]
    fn zero_weight_speaker_gets_zero_gain() {
        let src = [0.0, 0.0_f64];
        let speakers = vec![
            Speaker { distance_sq: dist(src, [1.0, 0.0]), weight: 1.0 },
            Speaker { distance_sq: dist(src, [5.0, 0.0]), weight: 0.0 },
        ];
        let gains: Vec<f64> = SpeakerGains::new(&speakers, 6.0).collect();
        assert_eq!(gains[1], 0.0);
    }

    #[test]
    fn rolloff_affects_gain_ratio() {
        // Two speakers at different distances — rolloff must change the near/far gain ratio.
        let src = [0.0, 0.0_f64];
        let near = dist(src, [1.0, 0.0]);
        let far  = dist(src, [4.0, 0.0]);
        let speakers = vec![
            Speaker { distance_sq: near, weight: 1.0 },
            Speaker { distance_sq: far,  weight: 1.0 },
        ];

        let gains_6db: Vec<f64> = SpeakerGains::new(&speakers, 6.0).collect();
        let gains_0db: Vec<f64> = SpeakerGains::new(&speakers, 0.0001).collect();

        let ratio_6 = gains_6db[0] / gains_6db[1];
        let ratio_0 = gains_0db[0] / gains_0db[1];

        assert!(
            ratio_6 > ratio_0 + 0.1,
            "rolloff_db has no effect on gain ratio: ratio_6={ratio_6:.4}, ratio_0={ratio_0:.4}"
        );
    }
}
