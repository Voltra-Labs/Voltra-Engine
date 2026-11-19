//! Writing an entity and its descendants to disk as a prefab.

use std::fmt;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::cycle::{nested_in, reaches};
use super::{fragment, Prefabs};
use crate::format::{ComponentRegistry, SceneError};

/// Why a prefab was not written.
#[derive(Debug)]
pub enum WriteError {
    /// Reading the entities or writing the file failed.
    Scene(SceneError),
    /// The selection holds an instance of `through`, which is `target` or
    /// holds one — writing it would make `target` contain itself.
    Cycle {
        target: AssetPath,
        through: AssetPath,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(e) => e.fmt(f),
            Self::Cycle { target, through } if target == through => {
                write!(f, "{} would contain an instance of itself", target.as_str())
            }
            Self::Cycle { target, through } => write!(
                f,
                "{} would contain itself through {}",
                target.as_str(),
                through.as_str()
            ),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Scene(e) => Some(e),
            Self::Cycle { .. } => None,
        }
    }
}

impl From<SceneError> for WriteError {
    fn from(e: SceneError) -> Self {
        Self::Scene(e)
    }
}

/// Writes `root` and its descendants to `target` under `prefabs`' root, and
/// refreshes the cached copy.
///
/// See [`fragment`] for what the file holds. Refused with nothing written
/// when it would nest `target` inside itself, directly or through another
/// prefab; see [`reaches`]. The write is atomic, like a scene save.
///
/// Refreshed here rather than left to the file watcher: whoever writes a
/// prefab is about to spawn it, and the watcher's event arrives frames later.
/// A path nothing has loaded is left unloaded.
pub fn write(
    world: &World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
    target: &AssetPath,
) -> Result<(), WriteError> {
    let file = fragment(world, registry, root)?;

    if let Some(through) = reaches(prefabs, nested_in(&file), target) {
        return Err(WriteError::Cycle {
            target: target.clone(),
            through,
        });
    }

    let path = prefabs.root().join(target.as_str());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SceneError::Io)?;
    }
    crate::format::write(&file, &path)?;
    prefabs.reload(target);
    Ok(())
}
