use cpal::traits::{DeviceTrait, StreamTrait};


/// A live cpal output stream.
pub struct Model {
    _stream: cpal::Stream,
}

impl Model {
    /// Build and start a cpal output stream on `device` that renders silence.
    ///
    /// Phase 2: renders silence. Phase 4+ will receive Sound updates via
    /// crossbeam and apply DBAP gains before writing to `data`.
    pub fn new(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
    ) -> anyhow::Result<Self> {
        let channels = config.channels as usize;
        let err_fn = |e| eprintln!("output stream error: {e}");
        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                render(data, channels);
            },
            err_fn,
            None,
        )?;
        stream.play()?;
        Ok(Model { _stream: stream })
    }
}

/// Audio render callback — fills `data` with silence.
///
/// Signature kept as a standalone fn so it can be unit-tested without a real device.
fn render(data: &mut [f32], _channels: usize) {
    for sample in data.iter_mut() {
        *sample = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::FRAMES_PER_BUFFER;

    #[test]
    fn render_writes_silence() {
        let mut buf = vec![1.0f32; FRAMES_PER_BUFFER * 2];
        render(&mut buf, 2);
        assert!(buf.iter().all(|&s| s == 0.0));
    }
}
