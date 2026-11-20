//! What an instance root remembers about the file it was spawned from.

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

/// The prefab an instance root was spawned from.
///
/// **Recorded and inert.** Nothing reads it back yet: an instance is still
/// plain entities, edited and saved like any other. It exists so the stage
/// that adds overrides and propagation finds every scene saved before it
/// already saying what each instance was made from — Unity's
/// `m_SourcePrefab`, Godot's `instance=ExtResource(...)`. Added after the
/// scenes are written, that link would have to be guessed.
///
/// On each **root** of an instance and nowhere else. A child of a root is
/// part of the same instance, and the root is where both engines keep the
/// link; a child carrying one of its own is a nested instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabInstance {
    pub source: AssetPath,
}

impl PrefabInstance {
    /// The name the component registry stores it under, which the code that
    /// reads one out of a file without spawning it has to agree with.
    pub const NAME: &'static str = "PrefabInstance";

    pub fn new(source: AssetPath) -> Self {
        Self { source }
    }
}
