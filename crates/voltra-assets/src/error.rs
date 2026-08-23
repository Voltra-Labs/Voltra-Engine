//! What can go wrong naming or loading an asset.

use std::fmt;
use std::path::PathBuf;

use voltra_audio::AudioError;
use voltra_render::TextureError;

/// A failure to name or load an asset.
///
/// The first three are rejections at construction and never reach a filesystem
/// call. `Read`, `Decode` and `Watch` are runtime failures: the first two
/// `Textures::load` turns into a warning and a placeholder rather than
/// propagating, and the third disables hot reload without stopping the app.
#[derive(Debug)]
pub enum AssetError {
    /// An absolute path, a volume prefix, or a UNC path.
    Absolute(String),
    /// A `..` component, which would name a file outside the asset root.
    EscapesRoot(String),
    /// A path with nothing left after normalisation.
    Empty,
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        source: TextureError,
    },
    /// A file that should have been RON was not.
    ///
    /// Boxed because a parse error carries the offending source line and its
    /// position, and inlining that widens every `Result` in the crate — which
    /// is `clippy::result_large_err`.
    Parse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },
    /// A sound that would not decode, or a file that is not one.
    ///
    /// Boxed for the same reason [`Self::Parse`] is: an `AudioError` carries a
    /// path and a decoder error of its own, and inlining it widens every
    /// `Result` in the crate.
    Audio {
        path: PathBuf,
        source: Box<AudioError>,
    },
    /// Valid RON that does not describe a slicing.
    Atlas {
        path: PathBuf,
        source: crate::atlas::AtlasError,
    },
    /// The filesystem watch could not be established.
    Watch {
        root: PathBuf,
        source: notify_debouncer_full::notify::Error,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absolute(raw) => {
                write!(
                    f,
                    "`{raw}` is absolute; asset paths are relative to the asset root"
                )
            }
            Self::EscapesRoot(raw) => {
                write!(f, "`{raw}` leaves the asset root")
            }
            Self::Empty => write!(f, "an asset path cannot be empty"),
            Self::Read { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            Self::Decode { path, source } => {
                write!(f, "could not decode {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "could not parse {}: {source}", path.display())
            }
            Self::Audio { path, source } => {
                write!(f, "could not load {}: {source}", path.display())
            }
            Self::Atlas { path, source } => {
                write!(f, "{} is not a usable atlas: {source}", path.display())
            }
            Self::Watch { root, source } => {
                write!(f, "could not watch {}: {source}", root.display())
            }
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Audio { source, .. } => Some(source),
            Self::Atlas { source, .. } => Some(source),
            Self::Watch { source, .. } => Some(source),
            Self::Absolute(_) | Self::EscapesRoot(_) | Self::Empty => None,
        }
    }
}
