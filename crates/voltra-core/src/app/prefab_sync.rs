//! Keeping the world's prefab instances in step with their files: once when
//! the world first meets the store, after each scene change, and when a
//! prefab is hot reloaded. See `voltra_scene::prefab::sync`.

use voltra_assets::AssetPath;
use voltra_scene::prefab;

use super::spawning::prefab_store;
use super::App;

impl App {
    /// Syncs every instance in the world, queueing what changed for its
    /// assets to be looked up again.
    pub(super) fn sync_all_instances(&mut self) {
        let prefabs = prefab_store(&mut self.prefabs, &self.asset_root);
        let report = prefab::sync_all(&mut self.world, &self.registry, prefabs);
        self.spawned.extend(report.touched);
    }

    /// Re-reads `path` if it is a prefab the store has loaded, and syncs its
    /// instances. Returns whether it was one.
    pub(super) fn reload_prefab(&mut self, path: &AssetPath) -> bool {
        let Some(prefabs) = self.prefabs.as_mut() else {
            return false;
        };
        if !prefabs.reload(path) {
            return false;
        }
        let report = prefab::sync_source(&mut self.world, &self.registry, prefabs, path);
        self.spawned.extend(report.touched);
        true
    }
}
