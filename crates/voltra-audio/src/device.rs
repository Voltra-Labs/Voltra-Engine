//! The output device, and the callback that feeds it.
//!
//! The whole of this crate's contact with the operating system, and
//! deliberately the thinnest part of it: the device is asked for a buffer
//! shape, and everything that decides what goes in the buffer is
//! [`Mixer`](crate::Mixer), which has no device and is therefore testable on a
//! machine with no sound card. There are no unit tests below for that reason —
//! what could be tested here is `cpal`, and what is ours is tested next door.

use std::sync::mpsc::{Receiver, Sender};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

use crate::clip::Clip;
use crate::command::Command;
use crate::error::AudioError;
use crate::mixer::Mixer;

/// A running output stream and the shape it runs at.
///
/// Dropping it stops the sound: `cpal` closes the stream with the handle, so
/// this is kept alive for exactly as long as audio should be audible.
pub struct Output {
    /// Never read. It exists to be dropped at the right time — see above —
    /// and every message for it goes down the command channel instead.
    _stream: cpal::Stream,
    rate: u32,
    channels: u16,
}

impl Output {
    /// Opens the default output device and starts feeding it from `commands`.
    ///
    /// Finished clips are sent back through `retired` so their memory is freed
    /// on the thread that asked for them rather than inside the callback.
    pub fn open(commands: Receiver<Command>, retired: Sender<Clip>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
        let supported = float_config(&device)?;

        let rate = supported.sample_rate();
        let channels = supported.channels();
        let config = StreamConfig {
            channels,
            sample_rate: rate,
            // Let the host pick. A fixed size is a latency choice, and this
            // engine has nothing yet that would justify making one — a wrong
            // guess is either an underrun or a delay on every sound.
            buffer_size: cpal::BufferSize::Default,
        };

        let mut mixer = Mixer::new(rate, channels).retiring_to(retired);

        let stream = device
            .build_output_stream(
                config,
                move |out: &mut [f32], _| {
                    // Everything the game said since the last buffer, applied
                    // before a sample is written: a sound triggered this frame
                    // starts in this buffer rather than the next.
                    //
                    // `try_iter` never blocks. A callback that waited on the
                    // game would miss its deadline and the speakers would get
                    // a gap — the one thing an audio thread must never do.
                    for command in commands.try_iter() {
                        mixer.apply(command);
                    }
                    mixer.render(out);
                },
                // Not fatal: `cpal` reports a device that was unplugged or a
                // buffer that arrived late, and there is nothing useful to do
                // beyond saying so. The stream stays open, and if it is really
                // gone the buffers simply stop.
                |e| log::error!("audio stream error: {e}"),
                None,
            )
            .map_err(|source| AudioError::Device(Box::new(source)))?;

        // Required on every backend since `cpal` 0.18 — before it, some
        // started on their own and some did not.
        stream
            .play()
            .map_err(|source| AudioError::Device(Box::new(source)))?;

        log::info!("audio: {channels} channels at {rate} Hz");

        Ok(Self {
            _stream: stream,
            rate,
            channels,
        })
    }

    /// Samples per second per channel the device runs at.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Samples per frame the device expects.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// The device's own configuration if it is already 32-bit float, else the
/// first float configuration it offers.
///
/// Everything downstream of here is `f32`, so a device that cannot take
/// floats is refused rather than converted: the conversion would be one more
/// buffer and one more format to test, and no host this engine targets is
/// without a float path — WASAPI, CoreAudio and PulseAudio all take floats
/// natively, and ALSA's `plug` layer converts before the hardware.
fn float_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, AudioError> {
    if let Ok(config) = device.default_output_config() {
        if config.sample_format() == SampleFormat::F32 {
            return Ok(config);
        }
    }

    device
        .supported_output_configs()
        .map_err(|source| AudioError::Device(Box::new(source)))?
        .filter(|range| range.sample_format() == SampleFormat::F32)
        .find_map(|range| range.try_with_standard_sample_rate())
        .ok_or(AudioError::NoFloatConfig)
}
