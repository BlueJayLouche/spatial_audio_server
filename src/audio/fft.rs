use rustfft::{num_complex::Complex, FftPlanner};

use super::SAMPLE_RATE;

pub const FFT_WINDOW_LEN: usize = 1024;
pub const FFT_BIN_STEP_HZ: f64 = SAMPLE_RATE / FFT_WINDOW_LEN as f64;

/// Hann window coefficients for spectral leakage reduction.
pub fn hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = std::f32::consts::PI * i as f32 / (n - 1) as f32;
            x.sin().powi(2)
        })
        .collect()
}

/// Forward FFT returning magnitude spectrum for the first N/2 positive-frequency bins.
///
/// `samples` must have exactly `FFT_WINDOW_LEN` elements. A Hann window is applied
/// before the transform to reduce spectral leakage.
pub fn forward(samples: &[f32]) -> Vec<f32> {
    assert_eq!(samples.len(), FFT_WINDOW_LEN, "FFT input must be exactly FFT_WINDOW_LEN samples");
    let window = hann_window(FFT_WINDOW_LEN);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_WINDOW_LEN);
    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .zip(window.iter())
        .map(|(&s, &w)| Complex { re: s * w, im: 0.0 })
        .collect();
    let mut scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
    fft.process_with_scratch(&mut buffer, &mut scratch);
    buffer[..FFT_WINDOW_LEN / 2].iter().map(|c| c.norm()).collect()
}

/// Frequency (Hz) of FFT magnitude bin `i`.
#[inline]
pub fn bin_to_hz(i: usize) -> f64 {
    i as f64 * FFT_BIN_STEP_HZ
}

/// Mel-scale conversion (O'Shaughnessy 1987) — replaces pitch_calc dependency.
#[inline]
pub fn hz_to_mel(hz: f64) -> f64 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

#[inline]
pub fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (10f64.powf(mel / 2595.0) - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_wave_peak_in_correct_bin() {
        let freq_hz = 1000.0_f32;
        let samples: Vec<f32> = (0..FFT_WINDOW_LEN)
            .map(|i| {
                (2.0 * std::f32::consts::PI * freq_hz * i as f32 / SAMPLE_RATE as f32).sin()
            })
            .collect();
        let mags = forward(&samples);
        let peak_bin = mags
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        let peak_hz = bin_to_hz(peak_bin);
        assert!(
            (peak_hz - freq_hz as f64).abs() < FFT_BIN_STEP_HZ,
            "Peak at {peak_hz:.1}Hz, expected ~{freq_hz}Hz (bin step = {FFT_BIN_STEP_HZ:.2}Hz)"
        );
    }

    #[test]
    fn mel_round_trip() {
        for hz in [100.0, 440.0, 1000.0, 4000.0, 16000.0] {
            let back = mel_to_hz(hz_to_mel(hz));
            assert!((back - hz).abs() < 1e-6, "mel round-trip failed for {hz}Hz: got {back}");
        }
    }

    #[test]
    fn bin_to_hz_zero_is_dc() {
        assert_eq!(bin_to_hz(0), 0.0);
    }
}
