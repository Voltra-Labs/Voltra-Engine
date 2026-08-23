//! What can go wrong decoding a clip or opening a device.

use std::fmt;
use std::path::PathBuf;

/// A failure to decode a sound or to reach the speakers.
///
/// `Read`, `Decode` and `Unsupported` are per file, and the store above turns
/// them into a warning and a silent clip rather than propagating — a scene
/// naming a sound that will not decode must still open, the same way a scene
/// naming a broken PNG still draws. `NoDevice` and `Device` are per session:
/// they disable audio and leave the app running.
#[derive(Debug)]
pub enum AudioError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Boxed because `symphonia`'s error carries an `io::Error` and a string,
    /// and inlining it widens every `Result` in the crate — `clippy`'s
    /// `result_large_err`.
    Decode {
        path: PathBuf,
        source: Box<symphonia::core::errors::Error>,
    },
    /// A file that decoded but describes nothing playable: no audio track, no
    /// codec parameters, no channels, or a zero sample rate.
    Unsupported { path: PathBuf, reason: &'static str },
    /// The host has no output device at all — a headless CI runner, a machine
    /// with the sound card disabled.
    NoDevice,
    /// A device that exists but would not give us a stream.
    Device(Box<cpal::Error>),
    /// A device that exists but offers no 32-bit float output configuration.
    ///
    /// Its own variant rather than a `Device` error because it is the one
    /// failure a user could act on, and because it is the assumption the
    /// mixer is built on: everything downstream of here is `f32`.
    NoFloatConfig,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            Self::Decode { path, source } => {
                write!(f, "could not decode {}: {source}", path.display())
            }
            Self::Unsupported { path, reason } => {
                write!(f, "{} is not playable: {reason}", path.display())
            }
            Self::NoDevice => write!(f, "no audio output device"),
            Self::Device(source) => write!(f, "could not open an audio stream: {source}"),
            Self::NoFloatConfig => {
                write!(f, "the output device offers no 32-bit float configuration")
            }
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Device(source) => Some(source),
            Self::Unsupported { .. } | Self::NoDevice | Self::NoFloatConfig => None,
        }
    }
}
