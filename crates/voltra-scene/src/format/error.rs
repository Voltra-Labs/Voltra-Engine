//! What can go wrong saving or loading a scene.

use std::fmt;

/// A failure while reading or writing a scene file.
///
/// The variants are separate because they call for different responses. A
/// missing file is the user's problem; malformed RON is the file's; a component
/// that fails to deserialize is *ours*, because the name being registered says
/// this build claims to understand it.
#[derive(Debug)]
pub enum SceneError {
    Io(std::io::Error),
    Parse(ron::error::SpannedError),
    Serialize(ron::Error),
    UnsupportedVersion {
        found: u32,
        supported: u32,
    },
    /// A registered component whose stored data does not fit its type.
    ///
    /// Deliberately not treated as an unknown component. Unknown means "not
    /// mine, do not touch"; this means "mine, and broken", and swallowing it
    /// would silently drop data on the next save.
    Component {
        name: String,
        source: ron::Error,
    },
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "scene file i/o failed: {e}"),
            Self::Parse(e) => write!(f, "scene file is not valid RON: {e}"),
            Self::Serialize(e) => write!(f, "could not write the scene: {e}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "scene file is version {found}, this build supports {supported}"
            ),
            Self::Component { name, source } => {
                write!(f, "component `{name}` could not be read: {source}")
            }
        }
    }
}

impl std::error::Error for SceneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Serialize(e) | Self::Component { source: e, .. } => Some(e),
            Self::UnsupportedVersion { .. } => None,
        }
    }
}

impl From<std::io::Error> for SceneError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
