//! A prefab read again — its file changed, or the registry the store reads
//! through did — and every prefab holding an instance of it read again
//! after it, so a variant and an outer prefab follow without being saved.

use std::collections::HashSet;

use voltra_assets::AssetPath;

use super::super::cycle::nested_in;
use super::Prefabs;
use crate::format::ComponentRegistry;

impl Prefabs {
    /// Re-reads `path` under the handle it already has, then every loaded
    /// prefab holding an instance of it — a variant of it, a prefab it is
    /// nested in, and so on outwards. Answers every path read again, `path`
    /// first: whose instances in a world need bringing in step
    /// ([`super::super::sync_sources`]). Empty when nothing changed.
    ///
    /// A path nothing loaded is ignored, and a file that will not read keeps
    /// the previous prefab, for the reason every other store gives: an
    /// editor mid-save leaves it truncated.
    pub fn reload(&mut self, path: &AssetPath) -> Vec<AssetPath> {
        let Some(&handle) = self.by_path.get(path) else {
            return Vec::new();
        };
        let stored = match self.read(path) {
            Ok(file) => file,
            Err(e) => {
                log::warn!(
                    "prefab {}: {e}; keeping the previously loaded one",
                    path.as_str()
                );
                return Vec::new();
            }
        };
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
    /// through another — nearest first, `path` itself left out.
    pub fn dependents(&self, path: &AssetPath) -> Vec<AssetPath> {
        let mut reached: HashSet<AssetPath> = HashSet::from([path.clone()]);
        let mut found = Vec::new();
        let cached: Vec<(AssetPath, Vec<AssetPath>)> = self
            .by_path
            .iter()
            .chain(self.stale.iter())
            .filter_map(|(at, &handle)| {
                let file = self.get(handle).stored_file()?;
                Some((at.clone(), nested_in(file)))
            })
            .collect();
        loop {
            let before = found.len();
            for (at, holds) in &cached {
                if !reached.contains(at) && holds.iter().any(|h| reached.contains(h)) {
                    reached.insert(at.clone());
                    found.push(at.clone());
                }
            }
            if found.len() == before {
                return found;
            }
        }
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
                self.get(handle)
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
