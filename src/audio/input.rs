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
        // Pre-allocate a scratch buffer so extend_from_slice never reallocates.
        // try_send avoids blocking the real-time callback when the consumer is slow.
        let mut scratch = Vec::<f32>::with_capacity(4096 * config.channels as usize);
        let stream = device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                scratch.clear();
                scratch.extend_from_slice(data);
                let _ = sample_tx.try_send(scratch.clone());
            },
            err_fn,
            None,
        )?;
        stream.play()?;
        Ok(Model { _stream: stream })
    }
}
