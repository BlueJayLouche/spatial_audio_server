use crossbeam::channel::Sender;
use cpal::traits::{DeviceTrait, StreamTrait};

/// A live cpal input stream plus a handle to the captured-sample channel.
pub struct Model {
    _stream: cpal::Stream,
}

impl Model {
    /// Build and start a cpal input stream on `device`.
    ///
    /// Captured interleaved f32 samples are forwarded to `sample_tx` in
    /// FRAMES_PER_BUFFER-sized chunks so the output thread can mix
    /// real-time audio sources without extra buffering.
    pub fn new(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        sample_tx: Sender<Vec<f32>>,
    ) -> anyhow::Result<Self> {
        let err_fn = |e| eprintln!("input stream error: {e}");
        let stream = device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // One allocation per callback — try_send drops silently when the
                // consumer is slow, avoiding any backpressure on the audio thread.
                let _ = sample_tx.try_send(data.to_vec());
            },
            err_fn,
            None,
        )?;
        stream.play()?;
        Ok(Model { _stream: stream })
    }
}
