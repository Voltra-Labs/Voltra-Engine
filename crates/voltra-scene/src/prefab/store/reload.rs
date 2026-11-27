//! A prefab read again — its file changed, or the registry the store reads
//! through did — and every prefab holding an instance of it read again
//! after it, so a variant and an outer prefab follow without being saved.

use std::collections::HashMap;

use voltra_assets::AssetPath;

use super::super::cycle::nested_in;
use super::super::Prefab;
use super::{Prefabs, MAX_PREFAB_BYTES};
use crate::format::ComponentRegistry;

impl Prefabs {
    /// Re-reads `path` under the handle it already has, then every loaded
    /// prefab holding an instance of it — a variant of it, a prefab it is
    /// nested in, and so on outwards. Answers every path read again, `path`
    /// first, then the rest in [`dependents`](Self::dependents)' order:
    /// whose instances in a world need bringing in step
    /// ([`super::super::sync_sources`]).
    ///
    /// Empty when nothing changed: a path nothing loaded, a file that reads
    /// as it did, or one that will not read — which keeps the previous
    /// prefab, for the reason every other store gives: an editor mid-save
    /// leaves it truncated. A file read as a scene's
    /// ([`load_scene`](Self::load_scene)) is read again as one, alone.
    pub fn reload(&mut self, path: &AssetPath) -> Vec<AssetPath> {
        let Some(&handle) = self.by_path.get(path) else {
            return Vec::new();
        };
        let resolved = self.get(handle).resolved;
        let limit = resolved.then_some(MAX_PREFAB_BYTES);
        let stored = match self.read(path, limit) {
            Ok(file) => file,
            Err(e) => {
                log::warn!(
                    "prefab {}: {e}; keeping the previously loaded one",
                    path.as_str()
                );
                return Vec::new();
            }
        };
        if self.get(handle).stored_file() == Some(&stored) {
            return Vec::new();
        }
        if !resolved {
            let prefab = Prefab {
                file: Some(stored),
                source: Some(path.clone()),
                ..Prefab::default()
            };
            self.put(path, prefab, Some(handle));
            log::info!("reloaded {}", path.as_str());
            return vec![path.clone()];
        }
        let dependents = self.dependents(path);
        for dependent in &dependents {
            if let Some(handle) = self.by_path.remove(dependent) {
                self.stale.insert(dependent.clone(), handle);
            }
        }
        self.by_path.remove(path);
        let prefab = self.resolved(path, stored, &mut Vec::new());
        self.put(path, prefab, Some(handle));
        // Each read from what it held, the files it holds instances of
        // first: a dependent reached on the way is already done.
        for dependent in &dependents {
            self.load_at(dependent, &mut Vec::new());
        }
        log::info!("reloaded {}", path.as_str());
        std::iter::once(path.clone()).chain(dependents).collect()
    }

    /// Every loaded prefab holding an instance of `path`, directly or
    /// through another, `path` itself left out — each after every one of
    /// them it holds an instance of (by its longest chain of holding back
    /// to `path`), and by path among equals, so the order is the same every
    /// time. A file read as a scene's is not one: its instances are synced
    /// in the world, by their own files.
    pub fn dependents(&self, path: &AssetPath) -> Vec<AssetPath> {
        let cached: HashMap<AssetPath, Vec<AssetPath>> = self
            .by_path
            .iter()
            .chain(self.stale.iter())
            .filter_map(|(at, &handle)| {
                let prefab = self.get(handle);
                if !prefab.resolved {
                    return None;
                }
                Some((at.clone(), nested_in(prefab.stored_file()?)))
            })
            .collect();
        // Rank: the longest chain of holding from `path`, worked out by
        // passes — at most one per prefab, so a loop ends.
        let mut rank: HashMap<&AssetPath, usize> = HashMap::from([(path, 0)]);
        for _ in 0..=cached.len() {
            let mut moved = false;
            for (at, holds) in &cached {
                if at == path {
                    continue;
                }
                let best = holds
                    .iter()
                    .filter_map(|h| rank.get(h))
                    .max()
                    .map(|r| r + 1);
                if let Some(best) = best {
                    let current = rank.get(at).copied();
                    if current.is_none_or(|c| c < best && best <= cached.len()) {
                        rank.insert(at, best);
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
        let mut found: Vec<(usize, &AssetPath)> = rank
            .into_iter()
            .filter(|(at, _)| *at != path)
            .map(|(at, r)| (r, at))
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.as_str().cmp(b.1.as_str())));
        found.into_iter().map(|(_, at)| at.clone()).collect()
    }

    /// Reads stored instances through `registry` from now on, if it is not
    /// the one already held (by [`ComponentRegistry::stamp`]) — a game's
    /// components swapped in or out — reading again every prefab that
    /// holds one. Answers the paths read again.
    pub fn use_registry(&mut self, registry: &ComponentRegistry) -> Vec<AssetPath> {
        if self.registry.stamp() == registry.stamp() {
            return Vec::new();
        }
        self.registry = registry.clone();
        let holding: Vec<AssetPath> = self
            .by_path
            .iter()
            .filter(|(_, &handle)| {
                let prefab = self.get(handle);
                prefab.resolved
                    && prefab
                        .stored_file()
                        .is_some_and(|file| !nested_in(file).is_empty())
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in &holding {
            if let Some(handle) = self.by_path.remove(path) {
                self.stale.insert(path.clone(), handle);
            }
        }
        for path in &holding {
            self.load_at(path, &mut Vec::new());
        }
        holding
    }
}
