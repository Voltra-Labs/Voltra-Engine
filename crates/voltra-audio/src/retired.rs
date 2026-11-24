//! What the audio thread hands back to be freed somewhere else.

use crate::clip::Clip;
use crate::stream::Feed;

/// Everything the mixer lets go of that could be the last reference to a
/// buffer.
///
/// Sent back to the main thread rather than dropped on the audio thread,
/// where freeing a megabyte of samples — or a stream's ring — would be a
/// pause in the middle of a buffer nobody can afford. The main thread drops
/// it in [`Audio::end_frame`](crate::Audio::end_frame).
#[derive(Debug)]
pub enum Retired {
    /// A voice that finished, was stopped, or was stolen.
    Voice {
        /// The clip it played.
        clip: Clip,
        /// The feed it read, for a streamed clip.
        feed: Option<Feed>,
    },
}

impl Retired {
    /// The clip, for a voice.
    pub fn clip(&self) -> Option<&Clip> {
        match self {
            Self::Voice { clip, .. } => Some(clip),
        }
    }
}
