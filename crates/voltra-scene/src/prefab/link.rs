//! What an instance remembers about the file it was spawned from: the link
//! on its root, and on every entity which record of the file it is and what
//! of it the instance has made its own.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

use crate::scene_id::SceneId;

/// The prefab an instance root was spawned from — Unity's `m_SourcePrefab`,
/// Godot's `instance=ExtResource(...)`.
///
/// On each **root** of an instance and nowhere else. A child of a root is
/// part of the same instance; a child carrying a link of its own is a nested
/// instance. What the root and everything under it are kept in step with is
/// this file: see [`super::sync`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabInstance {
    /// The prefab file the instance was spawned from.
    pub source: AssetPath,
    /// Records of the prefab this instance has deleted from itself: a sync
    /// does not spawn them back. By the prefab's own ids. Not written when
    /// empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<SceneId>,
}

impl PrefabInstance {
    /// The name the component registry stores it under, which the code that
    /// reads one out of a file without spawning it has to agree with.
    pub const NAME: &'static str = "PrefabInstance";

    /// An instance of `source` with nothing removed.
    pub fn new(source: AssetPath) -> Self {
        Self {
            source,
            removed: Vec::new(),
        }
    }
}

/// Which record of its instance's prefab an entity is, and which of its
/// components the instance overrides — Unity's corresponding source object
/// and its property modifications, by component rather than by field.
///
/// On every entity a prefab spawn made, root included. An entity under an
/// instance root without one was added to the instance and is the
/// instance's own. A sync leaves an overridden component as it is and puts
/// every other one back to what the prefab says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabPart {
    /// The record's id in the prefab file — not this entity's own
    /// [`SceneId`], which is fresh for every instance.
    pub record: SceneId,
    /// The components, by registered name, the instance has made its own:
    /// changed, added, or taken off. Not written when empty.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub overridden: BTreeSet<String>,
}

impl PrefabPart {
    /// The name the component registry stores it under.
    pub const NAME: &'static str = "PrefabPart";

    /// The part spawned from `record`, overriding nothing.
    pub fn new(record: SceneId) -> Self {
        Self {
            record,
            overridden: BTreeSet::new(),
        }
    }
}
