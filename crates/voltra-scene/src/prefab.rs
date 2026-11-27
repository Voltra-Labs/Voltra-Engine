//! Stamping a stored fragment of a scene into a world, as many times as wanted.
//!
//! Unity's prefabs, Godot's instanced scenes, Unreal's blueprint classes: all
//! three separate *the asset* — a template on disk — from *the instance*, the
//! entities a spawn made out of it. This is the spawn half, and the asset is
//! the format the engine already has. A prefab **is** a scene file: one or
//! more roots and whatever hangs off them, in the same `.ron` every scene is
//! saved as, read through the same [`ComponentRegistry`]. Nothing new has to
//! be able to read one, and any scene can be instanced.
//!
//! **An instance follows its file.** Each root carries a [`PrefabInstance`]
//! naming its file and every entity a [`PrefabPart`] naming its record and
//! the components the instance has made its own; [`sync`] puts every other
//! component back to what the file says, spawns what the file gained and
//! despawns what it lost. See `sync.rs`.

mod adopt;
mod apply;
mod compare;
mod cycle;
mod fragment;
mod instances;
mod link;
mod overrides;
mod spawn;
mod staged;
mod store;
mod sync;
mod write;

use voltra_assets::AssetPath;

use crate::format::{SceneError, SceneFile, VERSION};

pub use apply::{apply, link};
pub use cycle::reaches;
pub use fragment::fragment;
pub use instances::{sync_all, sync_source};
pub use link::{PrefabInstance, PrefabPart};
pub use overrides::{instance_root, note_removed, refresh_overrides, revert, revert_instance};
pub use spawn::{spawn, spawn_scene, Instance};
pub use staged::StagedScene;
pub use store::Prefabs;
pub use sync::{sync, SyncReport};
pub use write::{write, WriteError};

/// A scene fragment, read and checked once, ready to be spawned.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prefab {
    file: Option<SceneFile>,
    /// The path it was read from, which every root it spawns is linked to.
    /// `None` for one built in memory, whose instances have no file to name.
    source: Option<AssetPath>,
}

impl Prefab {
    /// A prefab holding `file`, or the version error that makes it unusable.
    ///
    /// Checked here rather than at every spawn: a bullet prefab is spawned a
    /// hundred times a second, and the answer does not change between them.
    pub fn new(file: SceneFile) -> Result<Self, SceneError> {
        if file.version != VERSION {
            return Err(SceneError::UnsupportedVersion {
                found: file.version,
                supported: VERSION,
            });
        }
        Ok(Self {
            file: Some(file),
            source: None,
        })
    }

    /// The same prefab, its instances linked to `source`.
    pub fn with_source(self, source: AssetPath) -> Self {
        Self {
            source: Some(source),
            ..self
        }
    }

    /// The path this prefab's instances are linked to, if it has one.
    pub fn source(&self) -> Option<&AssetPath> {
        self.source.as_ref()
    }

    /// The prefab that spawns nothing: what a file that will not read becomes.
    pub fn empty() -> Self {
        Self::default()
    }

    /// How many entities one spawn makes.
    pub fn len(&self) -> usize {
        self.file.as_ref().map_or(0, |file| file.entities.len())
    }

    /// Whether the prefab holds no entities.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests;
