use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

use super::fft::{hann_window, FFT_WINDOW_LEN};

/// 75% overlap: advance the window by one quarter of its length each hop.
/// At 48 kHz this gives ~187 spectrum updates per second instead of ~47.
const HOP_SIZE: usize = FFT_WINDOW_LEN / 4;

/// Rolling-window RMS envelope detector — allocation-free after construction.
pub struct EnvDetector {
    buffer: Vec<f32>,
    pos: usize,
    sum_sq: f32,
}

impl EnvDetector {
    pub fn new(window_len: usize) -> Self {
        EnvDetector { buffer: vec![0.0; window_len], pos: 0, sum_sq: 0.0 }
    }

    /// Push one sample into the rolling window.
    pub fn push(&mut self, sample: f32) {
        let old = self.buffer[self.pos];
        self.sum_sq = (self.sum_sq - old * old + sample * sample).max(0.0);
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();
    }

    /// Current RMS of the window.
    pub fn rms(&self) -> f32 {
        (self.sum_sq / self.buffer.len() as f32).sqrt()
    }
}

/// Accumulates audio samples and computes a magnitude spectrum on every hop.
///
/// All internal buffers are pre-allocated — no heap allocation in the push path.
/// The FFT is recomputed every `HOP_SIZE` samples once the ring buffer is filled
/// (75% overlap, giving ~187 Hz spectrum update rate at 48 kHz).
pub struct FftDetector {
    ring: Vec<f32>,
    /// Next write position; also the index of the oldest sample when filled.
    ring_pos: usize,
    filled: bool,
    window: Vec<f32>,
    work: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    fft: Arc<dyn rustfft::Fft<f32>>,
    /// Latest magnitude spectrum (FFT_WINDOW_LEN / 2 bins).
    bins: Vec<f32>,
}

impl FftDetector {
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_WINDOW_LEN);
        let scratch_len = fft.get_inplace_scratch_len();
        FftDetector {
            ring: vec![0.0; FFT_WINDOW_LEN],
            ring_pos: 0,
            filled: false,
            window: hann_window(FFT_WINDOW_LEN),
            work: vec![Complex::default(); FFT_WINDOW_LEN],
            scratch: vec![Complex::default(); scratch_len],
            fft,
            bins: vec![0.0; FFT_WINDOW_LEN / 2],
        }
    }

    /// Push one sample. The FFT is recomputed every `HOP_SIZE` samples once
    /// the ring buffer has been filled for the first time.
    pub fn push(&mut self, sample: f32) {
        self.ring[self.ring_pos] = sample;
        self.ring_pos += 1;
        if self.ring_pos == FFT_WINDOW_LEN {
            self.ring_pos = 0;
            self.filled = true;
        }
        if self.filled && self.ring_pos % HOP_SIZE == 0 {
            self.compute_fft();
        }
    }

    /// Latest magnitude bins; returns all-zero until the first full window arrives.
    pub fn bins(&self) -> &[f32] {
        &self.bins
    }

    fn compute_fft(&mut self) {
        // ring_pos is the next write position = index of the oldest sample.
        // Read FFT_WINDOW_LEN samples in chronological order (oldest → newest).
        let start = self.ring_pos;
        for i in 0..FFT_WINDOW_LEN {
            let ring_i = (start + i) % FFT_WINDOW_LEN;
            self.work[i].re = self.ring[ring_i] * self.window[i];
            self.work[i].im = 0.0;
        }
        self.fft.process_with_scratch(&mut self.work, &mut self.scratch);
        for (bin, c) in self.bins.iter_mut().zip(self.work.iter()) {
            *bin = c.norm();
        }
    }
}

impl Default for FftDetector {
    fn default() -> Self {
        Self::new()
    }
}
