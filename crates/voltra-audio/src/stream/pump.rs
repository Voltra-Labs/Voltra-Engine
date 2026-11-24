//! Every playing stream's decoder, topped up together.

use super::decoder::{Progress, StreamDecoder};

/// The decoders of every stream that is playing.
///
/// One `pump` tops every ring up and lets go of the decoders that are done —
/// a file that ended, or a voice that was stopped and dropped its feed. What
/// runs the pump is the platform's business: [`Streamer`](super::Streamer).
#[derive(Debug, Default)]
pub struct Pump {
    decoders: Vec<StreamDecoder>,
}

impl Pump {
    /// An empty pump.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts topping `decoder` up.
    pub fn add(&mut self, decoder: StreamDecoder) {
        self.decoders.push(decoder);
    }

    /// Fills every ring as far as it goes and drops what is done.
    pub fn pump(&mut self) {
        self.decoders
            .retain_mut(|decoder| decoder.fill() == Progress::Running);
    }

    /// How many streams are still being decoded.
    pub fn len(&self) -> usize {
        self.decoders.len()
    }

    /// Whether nothing is being decoded.
    pub fn is_empty(&self) -> bool {
        self.decoders.is_empty()
    }
}
