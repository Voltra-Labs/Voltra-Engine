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
    nested_counted(file).0
}

/// [`nested_in`], with how many records carry a link — what the store's
/// [`super::MAX_LINKS`] counts. Linear; a link that will not read is warned
/// of once per file, not once per link.
pub(super) fn nested_counted(file: &SceneFile) -> (Vec<AssetPath>, usize) {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut links = 0;
    let mut unreadable = 0usize;
    for record in &file.entities {
        let Some(link) = record.components.get(PrefabInstance::NAME) else {
            continue;
        };
        links += 1;
        match link.into_rust::<PrefabInstance>() {
            Ok(link) => {
                if seen.insert(link.source.clone()) {
                    found.push(link.source);
                }
            }
            // A link that does not read is a broken record a load would
            // already refuse; it is no evidence of a cycle either way.
            Err(_) => unreadable += 1,
        }
    }
    if unreadable > 0 {
        log::warn!("{unreadable} nested prefab link(s) could not be read");
    }
    (found, links)
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
