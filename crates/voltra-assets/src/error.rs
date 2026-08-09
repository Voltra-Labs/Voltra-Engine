//! What can go wrong naming or loading an asset.

use std::fmt;
use std::path::PathBuf;

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
            Self::Watch { source, .. } => Some(source),
            Self::Absolute(_) | Self::EscapesRoot(_) | Self::Empty => None,
        }
    }
}
