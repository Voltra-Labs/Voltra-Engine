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
//!
//! **A prefab can hold instances of others**, stored as a scene stores one:
//! a *variant* is a file whose root is an instance of its base with
//! overrides, and a prefab nested in another is an instance inside it. The
//! store reads such a file as what it spawns — each instance in it brought
//! in step with its own file first (`resolve.rs`) — so an edit to a base or
//! an inner file reaches the variant, the outer prefab and every instance
//! of them, but for what each layer overrides.

mod adopt;
mod apply;
mod compare;
mod cycle;
mod fragment;
mod instances;
mod link;
mod membership;
mod overrides;
mod resolve;
mod spawn;
mod staged;
mod store;
mod sync;
mod variant;
mod write;

use voltra_assets::AssetPath;

use crate::format::{SceneError, SceneFile, VERSION};

pub use apply::{apply, link};
pub use cycle::reaches;
pub use fragment::fragment;
pub use instances::{sync_all, sync_in, sync_source, sync_sources};
pub use link::{PrefabInstance, PrefabPart};
pub use membership::instance_root;
pub use overrides::{note_removed, refresh_overrides, revert, revert_instance};
pub use resolve::{ResolveProblem, MAX_DEPTH, MAX_RECORDS};
pub use spawn::{spawn, spawn_scene, Instance};
pub use staged::StagedScene;
pub use store::Prefabs;
pub use sync::{sync, SyncReport};
pub use variant::{variant, variant_name};
pub use write::{write, WriteError};

/// A scene fragment, read and checked once, ready to be spawned.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prefab {
    /// What a spawn copies: the file as read, with every instance stored in
    /// it brought in step with its own file.
    file: Option<SceneFile>,
    /// The file as it is on disk, when bringing its stored instances in
    /// step changed it — what reading it again after an inner file changes
    /// starts from. `None` when it is `file`.
    stored: Option<SceneFile>,
    /// The path it was read from, which every root it spawns is linked to.
    /// `None` for one built in memory, whose instances have no file to name.
    source: Option<AssetPath>,
    /// Why instances stored in it were left as the file has them.
    problems: Vec<ResolveProblem>,
    /// Whether the store followed its stored instances when it read it —
    /// not for a file read as a level's ([`Prefabs::load_scene`]).
    resolved: bool,
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
            ..Self::default()
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

    /// The prefab this one is a variant of: the file its root is an
    /// instance of, if it is one.
    pub fn base(&self) -> Option<AssetPath> {
        let file = self.stored_file()?;
        let root = file
            .entities
            .iter()
            .find(|r| compare::parent_record(r).is_none())?;
        let link = root.components.get(PrefabInstance::NAME)?;
        link.into_rust::<PrefabInstance>().ok().map(|l| l.source)
    }

    /// Why instances stored in this prefab were left as its file has them
    /// — a loop, a chain too deep, an inner file that would not follow —
    /// rather than brought in step with their own files. Empty for one
    /// that read whole.
    pub fn problems(&self) -> &[ResolveProblem] {
        &self.problems
    }

    /// The file as it is on disk: its stored instances as last saved.
    pub(super) fn stored_file(&self) -> Option<&SceneFile> {
        self.stored.as_ref().or(self.file.as_ref())
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
