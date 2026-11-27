//! Whether a prefab would end up containing an instance of itself.
//!
//! The store follows every instance a prefab holds — a variant's base, a
//! prefab nested inside — to bring it in step (`resolve.rs`), so a loop is
//! a file that can never be read whole. It is refused at every place one
//! can be made — `prefab::write`, Apply, a variant, a save in the editor —
//! as Unity and Godot refuse a prefab that contains itself at save; one
//! made by hand on disk is caught by the store, which bounds its walk and
//! reports the loop rather than recursing into it.

use std::collections::HashSet;

use voltra_assets::AssetPath;

use super::{Prefab, PrefabInstance, Prefabs};
use crate::format::SceneFile;

impl Prefab {
    /// The prefabs this one holds instances of — a variant's base among
    /// them — each once.
    pub fn nested_sources(&self) -> Vec<AssetPath> {
        self.file.as_ref().map_or_else(Vec::new, nested_in)
    }
}

/// Every prefab a link in `file` names, each once: a root's (a variant's
/// base) as much as a nested instance's, since the store follows both.
pub(super) fn nested_in(file: &SceneFile) -> Vec<AssetPath> {
    let mut found = Vec::new();
    for record in &file.entities {
        let Some(link) = record.components.get(PrefabInstance::NAME) else {
            continue;
        };
        match link.into_rust::<PrefabInstance>() {
            Ok(link) if !found.contains(&link.source) => found.push(link.source),
            Ok(_) => {}
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
