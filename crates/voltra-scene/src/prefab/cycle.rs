//! Whether a prefab would end up containing an instance of itself.
//!
//! Nothing recurses through a nested instance today — a spawn copies the
//! records a file holds, and a nested instance is records like any other — so
//! a cycle cannot hang a spawn. It is refused anyway, at the one place one can
//! be made: the stage that propagates prefab edits *will* walk these links,
//! and a scene saved with a loop in it before then is a file that stage has to
//! reject. Unity and Godot both refuse a prefab that contains itself at save.

use std::collections::HashSet;

use voltra_assets::AssetPath;

use super::{Prefab, PrefabInstance, Prefabs};
use crate::format::SceneFile;
use crate::hierarchy::Parent;

impl Prefab {
    /// The prefabs this one holds instances of.
    ///
    /// Only links on records parented inside the file: a root's own link is
    /// replaced by every spawn, so it says nothing about what this file
    /// contains.
    pub fn nested_sources(&self) -> Vec<AssetPath> {
        self.file.as_ref().map_or_else(Vec::new, nested_in)
    }
}

/// See [`Prefab::nested_sources`].
pub(super) fn nested_in(file: &SceneFile) -> Vec<AssetPath> {
    let ids: HashSet<_> = file.entities.iter().map(|record| record.id).collect();
    let mut found = Vec::new();

    for record in &file.entities {
        let Some(link) = record.components.get(PrefabInstance::NAME) else {
            continue;
        };
        let parented_inside = record
            .components
            .get("Parent")
            .and_then(|raw| raw.into_rust::<Parent>().ok())
            .is_some_and(|parent| ids.contains(&parent.id));
        if !parented_inside {
            continue;
        }
        match link.into_rust::<PrefabInstance>() {
            Ok(link) => found.push(link.source),
            // A link that does not read is a broken record a load would
            // already refuse; it is no evidence of a cycle either way.
            Err(e) => log::warn!("a nested prefab link could not be read: {e}"),
        }
    }

    found
}

/// The first prefab, from `sources` and everything they nest in turn, that is
/// `target` — the path through which writing `target` would contain itself.
///
/// Reads through `prefabs`, so a file on a path is read once however many
/// branches lead to it, and a file that will not read counts as nesting
/// nothing. Each path is visited once: a diamond (two branches nesting the
/// same coin) is walked once, and a loop that does not pass through `target`
/// — already on disk, hand-written — ends instead of spinning.
pub fn reaches(
    prefabs: &mut Prefabs,
    sources: impl IntoIterator<Item = AssetPath>,
    target: &AssetPath,
) -> Option<AssetPath> {
    let mut seen = HashSet::new();
    let mut frontier: Vec<(AssetPath, AssetPath)> = sources
        .into_iter()
        .map(|source| (source.clone(), source))
        .collect();

    while let Some((path, through)) = frontier.pop() {
        if &path == target {
            return Some(through);
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        let handle = prefabs.load(&path);
        for nested in prefabs.get(handle).nested_sources() {
            frontier.push((nested, through.clone()));
        }
    }

    None
}
