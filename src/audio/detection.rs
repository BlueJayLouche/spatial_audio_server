/// Per-speaker audio analysis snapshot sent to the GUI and OSC output thread.
#[derive(Clone, Debug, Default)]
pub struct Detection {
    pub rms: f32,
    pub fft: Vec<f32>,
}

/// Aggregate audio analysis for one installation, sent ~60×/s over OSC.
#[derive(Clone, Debug, Default)]
pub struct AudioFrameData {
    pub avg_peak: f32,
    pub avg_rms: f32,
    pub avg_fft: FftData,
    pub speakers: Vec<SpeakerData>,
}

/// Low/mid/high + 8-bin FFT summary — matches the original OSC payload format exactly.
#[derive(Clone, Debug, Default)]
pub struct FftData {
    pub lmh: [f32; 3],
    pub bins: [f32; 8],
}

#[derive(Clone, Debug)]
pub struct SpeakerData {
    pub peak: f32,
    pub rms: f32,
}
